//! Postgres-backed [`CapabilityLeaseStore`] implementation.
//!
//! Stores capability leases in `brassclaw_capability_leases` (V007).
//! Status values are lowercased variant names of [`CapabilityLeaseStatus`]
//! (no `#[serde(rename_all)]` on the enum — the app layer lowercases manually):
//! `Active→"active"`, `Claimed→"claimed"`, `Consumed→"consumed"`, `Revoked→"revoked"`.
//!
//! Expiry filtering is performed in the application-layer WHERE clause, NOT in
//! the partial index predicate (which only covers `status = 'active'`).
//!
//! ## Atomicity model
//!
//! Every mutation (`revoke`, `claim`, `consume`) runs inside a single
//! `deadpool_postgres` transaction:
//!
//! 1. `BEGIN`
//! 2. `SELECT "grant" … FOR UPDATE` — acquires a row-level lock and reads the
//!    current lease in one round-trip.
//! 3. Rust-side validation and mutation (same helpers used by the in-memory and
//!    filesystem stores).
//! 4. `UPDATE … WHERE id = $n AND tenant_id = $n AND user_id = $n` — writes
//!    both the SQL status column (lowercase) and the full serialized JSONB
//!    grant (serde variant names) in one statement, then checks exactly one
//!    row was affected before committing.
//! 5. `COMMIT`
//!
//! Two concurrent callers locking the same row block until the first commits or
//! rolls back, so the read-then-validate-then-write sequence is serialised by
//! PostgreSQL rather than a Rust-level mutex. `claim` no longer performs a
//! separate pre-read: `ensure_claimable` runs on the locked row.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_pg::PgPool;
use serde_json::Value;

use crate::{
    CapabilityGrantId, CapabilityLease, CapabilityLeaseError, CapabilityLeaseStatus,
    CapabilityLeaseStore, ExecutionContext, InvocationFingerprint, ensure_claimable,
    ensure_consumable,
};

fn map_pool(e: deadpool_postgres::PoolError) -> CapabilityLeaseError {
    CapabilityLeaseError::Persistence {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> CapabilityLeaseError {
    CapabilityLeaseError::Persistence {
        reason: e.to_string(),
    }
}

fn status_str(status: CapabilityLeaseStatus) -> &'static str {
    match status {
        CapabilityLeaseStatus::Active => "active",
        CapabilityLeaseStatus::Claimed => "claimed",
        CapabilityLeaseStatus::Consumed => "consumed",
        CapabilityLeaseStatus::Revoked => "revoked",
    }
}

fn lease_from_value(payload: Value) -> Result<CapabilityLease, CapabilityLeaseError> {
    serde_json::from_value(payload).map_err(|e| CapabilityLeaseError::Persistence {
        reason: e.to_string(),
    })
}

fn lease_to_value(lease: &CapabilityLease) -> Result<Value, CapabilityLeaseError> {
    serde_json::to_value(lease).map_err(|e| CapabilityLeaseError::Persistence {
        reason: e.to_string(),
    })
}

/// Postgres-backed [`CapabilityLeaseStore`].
pub struct PgCapabilityLeaseStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgCapabilityLeaseStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    fn owns_scope(&self, scope: &ResourceScope) -> bool {
        scope.tenant_id.as_str() == self.tenant_id
    }

    /// Read a lease without any row lock. Used only by `get` and
    /// `leases_for_scope` / `active_leases_for_context` (non-mutating paths).
    async fn read_lease(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Result<Option<CapabilityLease>, CapabilityLeaseError> {
        if !self.owns_scope(scope) {
            return Ok(None);
        }
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT \"grant\" FROM brassclaw_capability_leases \
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3",
                &[
                    &lease_id.as_uuid().to_string(),
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
                let lease = lease_from_value(payload)?;
                Ok(crate::same_lease_scope(&lease.scope, scope).then_some(lease))
            }
        }
    }

    /// Perform a single atomic read-validate-mutate-write cycle inside a
    /// Postgres transaction with a row-level `FOR UPDATE` lock.
    ///
    /// The `mutate` closure receives the deserialized [`CapabilityLease`] and
    /// may modify it in place, returning an error if validation fails. On
    /// success the closure's mutations are persisted atomically; on any error
    /// the transaction is rolled back before the error is returned to the caller.
    ///
    /// The `UPDATE` predicate includes `id`, `tenant_id`, and `user_id` so a
    /// row with the same `id` owned by a different tenant or user can never be
    /// accidentally mutated.
    async fn mutate_lease<F>(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
        mutate: F,
    ) -> Result<CapabilityLease, CapabilityLeaseError>
    where
        F: FnOnce(&mut CapabilityLease) -> Result<(), CapabilityLeaseError>,
    {
        if !self.owns_scope(scope) {
            return Err(CapabilityLeaseError::UnknownLease { lease_id });
        }
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;

        // Lock the row for the duration of the transaction.
        let row = tx
            .query_opt(
                "SELECT \"grant\" FROM brassclaw_capability_leases \
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3 \
                 FOR UPDATE",
                &[
                    &lease_id.as_uuid().to_string(),
                    &self.tenant_id,
                    &scope.user_id.to_string(),
                ],
            )
            .await
            .map_err(map_pg)?;

        let Some(row) = row else {
            // Roll back the transaction (implicit on drop) and report an unknown
            // lease — row outside this caller's authority scope is indistinguishable
            // from a row that does not exist.
            return Err(CapabilityLeaseError::UnknownLease { lease_id });
        };

        let payload: Value = row.get(0);
        let mut lease = lease_from_value(payload)?;

        // Validate that the embedded lease scope exactly matches the requested scope.
        // A path-rewriting bug could route the wrong row here; surface it as
        // UnknownLease rather than silently mutating a different lease.
        if !crate::same_lease_scope(&lease.scope, scope) {
            return Err(CapabilityLeaseError::UnknownLease { lease_id });
        }

        // Apply the caller-supplied mutation while the row is locked.
        mutate(&mut lease)?;

        let new_status_col = status_str(lease.status);
        let updated_payload = lease_to_value(&lease)?;

        let rows_affected = tx
            .execute(
                "UPDATE brassclaw_capability_leases \
                 SET status = $1, \"grant\" = $2, updated_at = now() \
                 WHERE id = $3 AND tenant_id = $4 AND user_id = $5",
                &[
                    &new_status_col,
                    &updated_payload,
                    &lease_id.as_uuid().to_string(),
                    &self.tenant_id,
                    &scope.user_id.to_string(),
                ],
            )
            .await
            .map_err(map_pg)?;

        // The row was locked in this transaction; zero affected rows after a
        // successful lock is an invariant violation, not a normal error path.
        if rows_affected != 1 {
            return Err(CapabilityLeaseError::Persistence {
                reason: format!(
                    "pg capability lease store: UPDATE affected {rows_affected} rows \
                     for lease {lease_id} (expected exactly 1)"
                ),
            });
        }

        tx.commit().await.map_err(map_pg)?;
        Ok(lease)
    }
}

#[async_trait]
impl CapabilityLeaseStore for PgCapabilityLeaseStore {
    async fn issue(&self, lease: CapabilityLease) -> Result<CapabilityLease, CapabilityLeaseError> {
        if !self.owns_scope(&lease.scope) {
            return Err(CapabilityLeaseError::Persistence {
                reason: format!(
                    "pg capability lease store tenant {} does not own lease tenant {}",
                    self.tenant_id, lease.scope.tenant_id
                ),
            });
        }
        let payload = lease_to_value(&lease)?;
        let status_col = status_str(lease.status);
        let user_id = lease.scope.user_id.to_string();
        let capability_id = lease.grant.capability.to_string();
        let fingerprint = lease
            .invocation_fingerprint
            .as_ref()
            .map(|f| f.as_str().to_string());
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows_affected = client
            .execute(
                "INSERT INTO brassclaw_capability_leases \
                 (id, tenant_id, user_id, capability_id, status, \"grant\", invocation_fingerprint) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (id) DO NOTHING",
                &[
                    &lease.grant.id.as_uuid().to_string(),
                    &self.tenant_id,
                    &user_id,
                    &capability_id,
                    &status_col,
                    &payload,
                    &fingerprint,
                ],
            )
            .await
            .map_err(map_pg)?;

        // ON CONFLICT DO NOTHING silently inserts zero rows when the id already
        // exists. Returning the caller's lease in that case would claim success
        // for a row that was never persisted. Capability-grant IDs are UUIDs:
        // a duplicate is a persistence bug, not a normal duplicate-key race.
        if rows_affected == 0 {
            return Err(CapabilityLeaseError::Persistence {
                reason: format!(
                    "pg capability lease store: issue for lease {} had a conflicting id \
                     (zero rows inserted)",
                    lease.grant.id,
                ),
            });
        }

        Ok(lease)
    }

    async fn revoke(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        self.mutate_lease(scope, lease_id, |lease| {
            // Idempotent: revoking an already-revoked lease is a no-op.
            lease.status = CapabilityLeaseStatus::Revoked;
            Ok(())
        })
        .await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Option<CapabilityLease> {
        self.read_lease(scope, lease_id).await.ok().flatten()
    }

    async fn claim(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
        invocation_fingerprint: &InvocationFingerprint,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        // `ensure_claimable` runs inside `mutate_lease` on the FOR UPDATE–locked
        // row, eliminating the double-read race that existed in the previous
        // implementation (read for ensure_claimable + separate read in
        // transition_status). Two concurrent claimers will block on the row lock;
        // the second will see the Claimed status and return InactiveLease.
        self.mutate_lease(scope, lease_id, |lease| {
            ensure_claimable(lease, invocation_fingerprint)?;
            lease.status = CapabilityLeaseStatus::Claimed;
            Ok(())
        })
        .await
    }

    async fn consume(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        // Mirrors the InMemoryCapabilityLeaseStore::consume transition exactly:
        // - calls ensure_consumable (checks Active/Claimed status, expiry, exhaustion)
        // - for fingerprinted leases: zeroes max_invocations and sets Consumed
        // - for multi-use leases: decrements max_invocations; sets Consumed at 0
        // - for was_claimed leases with remaining invocations: resets to Active
        self.mutate_lease(scope, lease_id, |lease| {
            let was_claimed = lease.status == CapabilityLeaseStatus::Claimed;
            ensure_consumable(lease)?;
            if lease.invocation_fingerprint.is_some() {
                if let Some(remaining) = lease.grant.constraints.max_invocations.as_mut() {
                    *remaining = 0;
                }
                lease.status = CapabilityLeaseStatus::Consumed;
            } else if let Some(remaining) = lease.grant.constraints.max_invocations.as_mut() {
                *remaining -= 1;
                if *remaining == 0 {
                    lease.status = CapabilityLeaseStatus::Consumed;
                } else if was_claimed {
                    lease.status = CapabilityLeaseStatus::Active;
                }
            } else if was_claimed {
                lease.status = CapabilityLeaseStatus::Active;
            }
            Ok(())
        })
        .await
    }

    async fn leases_for_scope(&self, scope: &ResourceScope) -> Vec<CapabilityLease> {
        if !self.owns_scope(scope) {
            return Vec::new();
        }
        let user_id = scope.user_id.to_string();
        let client = match self.pool.get().await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let rows = match client
            .query(
                "SELECT \"grant\" FROM brassclaw_capability_leases \
                 WHERE tenant_id = $1 AND user_id = $2 \
                 ORDER BY created_at DESC",
                &[&self.tenant_id, &user_id],
            )
            .await
        {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        rows.into_iter()
            .filter_map(|r| {
                let payload: Value = r.get(0);
                lease_from_value(payload).ok()
            })
            .filter(|lease| crate::same_scope_owner(&lease.scope, scope))
            .collect()
    }

    async fn active_leases_for_context(&self, context: &ExecutionContext) -> Vec<CapabilityLease> {
        if !self.owns_scope(&context.resource_scope) {
            return Vec::new();
        }
        let user_id = context.user_id.to_string();
        let client = match self.pool.get().await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        // Expiry filter in WHERE clause (not in partial index predicate — see §4.8 C1 note).
        let rows = match client
            .query(
                "SELECT \"grant\" FROM brassclaw_capability_leases \
                 WHERE tenant_id = $1 AND user_id = $2 AND status = 'active' \
                 ORDER BY created_at DESC",
                &[&self.tenant_id, &user_id],
            )
            .await
        {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        rows.into_iter()
            .filter_map(|r| {
                let payload: Value = r.get(0);
                lease_from_value(payload).ok()
            })
            .filter(|lease| crate::same_scope_owner(&lease.scope, &context.resource_scope))
            .filter(|lease| crate::lease_is_authorizing(lease, context))
            .collect()
    }
}
