//! DB-backed implementation of [`ToolRegistryStore`] reading from `reborn_tools`.
//!
//! Only available behind the `skills-db` feature.
//!
//! # What is returned
//!
//! - `validation_status = 'validated'` rows only.
//! - Validation status is the delivery gate; consumer tags select recipients.
//! - `class_code = 0` (Rusty-only) — tools do not carry Monty/LLM prompt text.
//! - Filtered by the full `(tenant_id, user_id, agent_id, project_id)` scope
//!   tuple so that a wrong-scope read returns an empty set (scope isolation
//!   contract), **except** the builtin union (Phase C.2): `source = 'system'`
//!   validated rows are returned tenant-globally — tenant-anchored ($1) but
//!   agnostic on user_id/agent_id/project_id — so seeded `host.*` builtins are
//!   discoverable by every turn under the tenant. Tenant isolation is preserved
//!   (no cross-tenant leak).

use async_trait::async_trait;
use brassclaw_capabilities::tool_registry::{ToolRegistryError, ToolRegistryStore, ToolScopeKey};
use brassclaw_pg::PgPool;

/// [`ToolRegistryStore`] implementation backed by the `reborn_tools` PG table.
pub struct DbToolSource {
    pool: PgPool,
}

impl DbToolSource {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ToolRegistryStore for DbToolSource {
    async fn fetch_tool_names(
        &self,
        scope: &ToolScopeKey,
    ) -> Result<Vec<String>, ToolRegistryError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| ToolRegistryError::QueryFailed {
                reason: e.to_string(),
            })?;

        // Validation status is the delivery gate. Consumer tags are routing
        // metadata and do not override a validated status.
        let rows = client
            .query(
                "SELECT name
                 FROM reborn_tools
                 WHERE tenant_id   = $1
                   AND class_code  = 0
                   AND validation_status = 'validated'
                   AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                         OR source = 'system' )
                 ORDER BY prompt_uid ASC",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(|e| ToolRegistryError::QueryFailed {
                reason: e.to_string(),
            })?;

        let names = rows.into_iter().map(|r| r.get::<_, String>(0)).collect();
        Ok(names)
    }
}
