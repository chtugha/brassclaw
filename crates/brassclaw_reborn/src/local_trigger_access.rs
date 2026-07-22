//! Local-dev trigger-fire access store.
//!
//! `PgRebornLocalTriggerAccessStore` is the PostgreSQL-backed implementation;
//! it targets the `brassclaw_local_access` table created by V021 + V045.
//!
//! This table is not the production agent/project membership source of truth.
//! Production and multi-tenant runtimes must wire a real membership-backed
//! trigger access checker instead of this local bootstrap store.

use std::collections::HashSet;

use brassclaw_host_api::{AgentId, ProjectId, TenantId, UserId};
use chrono::Utc;
use thiserror::Error;

/// Fixed local-dev access role persisted on trigger-fire access rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTriggerAccessRole {
    /// Owner-level local trigger-fire access.
    Owner,
}

impl LocalTriggerAccessRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
        }
    }
}

/// Local-dev bootstrap path that owns a trigger-fire access row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTriggerAccessSource {
    /// Environment-token `serve` bootstrap path.
    LocalDevEnvBootstrap,
    /// SSO-admitted WebUI user bootstrap path.
    LocalDevSsoBootstrap,
    /// CLI `run` default-owner bootstrap path.
    LocalDevRunBootstrap,
}

impl LocalTriggerAccessSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::LocalDevEnvBootstrap => "local_dev_env_bootstrap",
            Self::LocalDevSsoBootstrap => "local_dev_sso_bootstrap",
            Self::LocalDevRunBootstrap => "local_dev_run_bootstrap",
        }
    }
}

/// Fixed lifecycle state persisted on local-dev access rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalTriggerAccessStatus {
    Active,
    Inactive,
}

impl LocalTriggerAccessStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

/// Failure modes of the local trigger access store.
#[derive(Debug, Error)]
pub enum RebornLocalTriggerAccessStoreError {
    #[error("reborn local trigger access store backend failure: {0}")]
    Backend(String),
}

/// Local-dev trigger access row to seed from trusted host/operator input.
pub struct LocalTriggerAccessSeed<'a> {
    pub tenant_id: &'a TenantId,
    pub user_id: &'a UserId,
    pub agent_id: Option<&'a AgentId>,
    pub project_id: Option<&'a ProjectId>,
    pub role: LocalTriggerAccessRole,
    pub source: LocalTriggerAccessSource,
}

/// Current trusted local-dev access set for one bootstrap source and exact
/// tenant/agent/project scope.
pub struct LocalTriggerAccessReconciliation<'a> {
    pub tenant_id: &'a TenantId,
    pub user_ids: &'a [UserId],
    pub agent_id: Option<&'a AgentId>,
    pub project_id: Option<&'a ProjectId>,
    pub role: LocalTriggerAccessRole,
    pub source: LocalTriggerAccessSource,
}

fn optional_scope_key(value: Option<&str>) -> &str {
    value.unwrap_or("")
}

// ── PgRebornLocalTriggerAccessStore ──────────────────────────────────────

/// PostgreSQL-backed local-dev trigger access repository.
///
/// Targets the `brassclaw_local_access` table created by V021.
pub struct PgRebornLocalTriggerAccessStore {
    pool: deadpool_postgres::Pool,
}

impl PgRebornLocalTriggerAccessStore {
    pub fn new(pool: deadpool_postgres::Pool) -> Self {
        Self { pool }
    }

    async fn connect(
        &self,
    ) -> Result<deadpool_postgres::Object, RebornLocalTriggerAccessStoreError> {
        self.pool
            .get()
            .await
            .map_err(|error| RebornLocalTriggerAccessStoreError::Backend(error.to_string()))
    }

    pub async fn seed_local_access(
        &self,
        seed: LocalTriggerAccessSeed<'_>,
    ) -> Result<(), RebornLocalTriggerAccessStoreError> {
        let now = Utc::now();
        let agent_key = optional_scope_key(seed.agent_id.map(AgentId::as_str));
        let project_key = optional_scope_key(seed.project_id.map(ProjectId::as_str));
        let client = self.connect().await?;
        client
            .execute(
                "INSERT INTO brassclaw_local_access \
                     (tenant_id, user_id, agent_id, project_id, role, status, source, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) \
                     ON CONFLICT (tenant_id, user_id, agent_id, project_id) DO NOTHING",
                &[
                    &seed.tenant_id.as_str(),
                    &seed.user_id.as_str(),
                    &agent_key,
                    &project_key,
                    &seed.role.as_str(),
                    &LocalTriggerAccessStatus::Active.as_str(),
                    &seed.source.as_str(),
                    &now,
                ],
            )
            .await
            .map_err(|error| RebornLocalTriggerAccessStoreError::Backend(error.to_string()))?;
        Ok(())
    }

    pub async fn reconcile_local_access(
        &self,
        reconciliation: LocalTriggerAccessReconciliation<'_>,
    ) -> Result<(), RebornLocalTriggerAccessStoreError> {
        let now = Utc::now();
        let agent_key = optional_scope_key(reconciliation.agent_id.map(AgentId::as_str));
        let project_key = optional_scope_key(reconciliation.project_id.map(ProjectId::as_str));
        // HashSet gives O(1) average contains() vs O(log n) for BTreeSet;
        // ordering is not needed here.
        let allowed: HashSet<&str> = reconciliation.user_ids.iter().map(UserId::as_str).collect();
        let client = self.connect().await?;

        let rows = client
            .query(
                "SELECT user_id \
                 FROM brassclaw_local_access \
                 WHERE tenant_id = $1 AND agent_id = $2 AND project_id = $3 \
                   AND source = $4 AND status = $5",
                &[
                    &reconciliation.tenant_id.as_str(),
                    &agent_key,
                    &project_key,
                    &reconciliation.source.as_str(),
                    &LocalTriggerAccessStatus::Active.as_str(),
                ],
            )
            .await
            .map_err(|error| RebornLocalTriggerAccessStoreError::Backend(error.to_string()))?;

        let mut stale_user_ids = Vec::new();
        for row in &rows {
            let user_id: String = row
                .try_get("user_id")
                .map_err(|error| RebornLocalTriggerAccessStoreError::Backend(error.to_string()))?;
            if !allowed.contains(user_id.as_str()) {
                stale_user_ids.push(user_id);
            }
        }

        for user_id in stale_user_ids {
            client
                .execute(
                    "UPDATE brassclaw_local_access SET status = $1, updated_at = $2 \
                     WHERE tenant_id = $3 AND user_id = $4 AND agent_id = $5 \
                       AND project_id = $6 AND source = $7 AND status = $8",
                    &[
                        &LocalTriggerAccessStatus::Inactive.as_str(),
                        &now,
                        &reconciliation.tenant_id.as_str(),
                        &user_id.as_str(),
                        &agent_key,
                        &project_key,
                        &reconciliation.source.as_str(),
                        &LocalTriggerAccessStatus::Active.as_str(),
                    ],
                )
                .await
                .map_err(|error| RebornLocalTriggerAccessStoreError::Backend(error.to_string()))?;
        }

        for user_id in reconciliation.user_ids {
            client
                .execute(
                    "INSERT INTO brassclaw_local_access \
                         (tenant_id, user_id, agent_id, project_id, role, status, source, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) \
                         ON CONFLICT (tenant_id, user_id, agent_id, project_id) DO NOTHING",
                    &[
                        &reconciliation.tenant_id.as_str(),
                        &user_id.as_str(),
                        &agent_key,
                        &project_key,
                        &reconciliation.role.as_str(),
                        &LocalTriggerAccessStatus::Active.as_str(),
                        &reconciliation.source.as_str(),
                        &now,
                    ],
                )
                .await
                .map_err(|error| {
                    RebornLocalTriggerAccessStoreError::Backend(error.to_string())
                })?;
        }
        Ok(())
    }

    pub async fn has_active_local_access(
        &self,
        tenant_id: &TenantId,
        user_id: &UserId,
        agent_id: Option<&AgentId>,
        project_id: Option<&ProjectId>,
    ) -> Result<bool, RebornLocalTriggerAccessStoreError> {
        let agent_key = optional_scope_key(agent_id.map(AgentId::as_str));
        let project_key = optional_scope_key(project_id.map(ProjectId::as_str));
        let client = self.connect().await?;
        let row = client
            .query_opt(
                "SELECT 1 FROM brassclaw_local_access \
                 WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 \
                   AND project_id = $4 AND status = $5 LIMIT 1",
                &[
                    &tenant_id.as_str(),
                    &user_id.as_str(),
                    &agent_key,
                    &project_key,
                    &LocalTriggerAccessStatus::Active.as_str(),
                ],
            )
            .await
            .map_err(|error| RebornLocalTriggerAccessStoreError::Backend(error.to_string()))?;
        Ok(row.is_some())
    }
}
