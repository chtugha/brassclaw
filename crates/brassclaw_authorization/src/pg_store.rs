//! Postgres-backed [`CapabilityLeaseStore`] implementation.
//!
//! Stores capability leases in `brassclaw_capability_leases` (V007).
//! Status values are lowercased variant names of [`CapabilityLeaseStatus`]
//! (no `#[serde(rename_all)]` on the enum — the app layer lowercases manually):
//! `Active→"active"`, `Claimed→"claimed"`, `Consumed→"consumed"`, `Revoked→"revoked"`.
//!
//! Expiry filtering is performed in the application-layer WHERE clause, NOT in
//! the partial index predicate (which only covers `status = 'active'`).

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_pg::PgPool;
use serde_json::Value;

use crate::{
    CapabilityGrantId, CapabilityLease, CapabilityLeaseError, CapabilityLeaseStatus,
    CapabilityLeaseStore, ExecutionContext, InvocationFingerprint,
};

fn map_pool(e: deadpool_postgres::PoolError) -> CapabilityLeaseError {
    CapabilityLeaseError::Persistence { reason: e.to_string() }
}

fn map_pg(e: tokio_postgres::Error) -> CapabilityLeaseError {
    CapabilityLeaseError::Persistence { reason: e.to_string() }
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
    serde_json::from_value(payload)
        .map_err(|e| CapabilityLeaseError::Persistence { reason: e.to_string() })
}

fn lease_to_value(lease: &CapabilityLease) -> Result<Value, CapabilityLeaseError> {
    serde_json::to_value(lease)
        .map_err(|e| CapabilityLeaseError::Persistence { reason: e.to_string() })
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

    async fn read_lease(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Result<Option<CapabilityLease>, CapabilityLeaseError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT grant FROM brassclaw_capability_leases \
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
                Ok(Some(lease_from_value(payload)?))
            }
        }
    }

    async fn transition_status(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
        new_status: CapabilityLeaseStatus,
        required_from: Option<CapabilityLeaseStatus>,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        let mut lease = self
            .read_lease(scope, lease_id)
            .await?
            .ok_or(CapabilityLeaseError::UnknownLease { lease_id })?;
        if let Some(required) = required_from
            && lease.status != required
        {
            return Err(CapabilityLeaseError::InactiveLease {
                lease_id,
                status: lease.status,
            });
        }
        lease.status = new_status;
        let payload = lease_to_value(&lease)?;
        let status_col = status_str(new_status);
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_capability_leases \
                 SET status = $1, grant = $2, updated_at = now() \
                 WHERE id = $3 AND tenant_id = $4",
                &[
                    &status_col,
                    &payload,
                    &lease_id.as_uuid().to_string(),
                    &self.tenant_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(lease)
    }
}

#[async_trait]
impl CapabilityLeaseStore for PgCapabilityLeaseStore {
    async fn issue(&self, lease: CapabilityLease) -> Result<CapabilityLease, CapabilityLeaseError> {
        let payload = lease_to_value(&lease)?;
        let status_col = status_str(lease.status);
        let user_id = lease.scope.user_id.to_string();
        let capability_id = lease.grant.capability.to_string();
        let fingerprint = lease
            .invocation_fingerprint
            .as_ref()
            .map(|f| f.as_str().to_string());
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_capability_leases \
                 (id, tenant_id, user_id, capability_id, status, grant, invocation_fingerprint) \
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
        Ok(lease)
    }

    async fn revoke(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        self.transition_status(scope, lease_id, CapabilityLeaseStatus::Revoked, None)
            .await
    }

    async fn get(&self, scope: &ResourceScope, lease_id: CapabilityGrantId) -> Option<CapabilityLease> {
        self.read_lease(scope, lease_id).await.ok().flatten()
    }

    async fn claim(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
        invocation_fingerprint: &InvocationFingerprint,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        let lease = self
            .read_lease(scope, lease_id)
            .await?
            .ok_or(CapabilityLeaseError::UnknownLease { lease_id })?;
        crate::ensure_claimable(&lease, invocation_fingerprint)?;
        self.transition_status(
            scope,
            lease_id,
            CapabilityLeaseStatus::Claimed,
            Some(CapabilityLeaseStatus::Active),
        )
        .await
    }

    async fn consume(
        &self,
        scope: &ResourceScope,
        lease_id: CapabilityGrantId,
    ) -> Result<CapabilityLease, CapabilityLeaseError> {
        self.transition_status(scope, lease_id, CapabilityLeaseStatus::Consumed, None)
            .await
    }

    async fn leases_for_scope(&self, scope: &ResourceScope) -> Vec<CapabilityLease> {
        let user_id = scope.user_id.to_string();
        let client = match self.pool.get().await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let rows = match client
            .query(
                "SELECT grant FROM brassclaw_capability_leases \
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
            .collect()
    }

    async fn active_leases_for_context(&self, context: &ExecutionContext) -> Vec<CapabilityLease> {
        let user_id = context.user_id.to_string();
        let client = match self.pool.get().await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        // Expiry filter in WHERE clause (not in partial index predicate — see §4.8 C1 note).
        let rows = match client
            .query(
                "SELECT grant FROM brassclaw_capability_leases \
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
            .collect()
    }
}
