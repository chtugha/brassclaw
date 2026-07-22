//! DB-backed implementation of [`ToolRegistryStore`] reading from `reborn_tools`.
//!
//! Only available behind the `skills-db` feature.
//!
//! # What is returned
//!
//! - `validation_status = 'validated'` rows only.
//! - Rows still carrying the `05:validator` consumer tag are **excluded**
//!   (the tag greys out delivery — §3.5.1).
//! - `class_code = 0` (Rusty-only) — tools do not carry Monty/LLM prompt text.
//! - Filtered by the full `(tenant_id, user_id, agent_id, project_id)` scope
//!   tuple so that a wrong-scope read returns an empty set (scope isolation
//!   contract).

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

        // Return validated Rusty tools for the scope, excluding any row that
        // still carries the '05:validator' consumer tag (greyed-out delivery).
        //
        // The NOT ('05:validator' = ANY(consumer_tags)) predicate is the DB
        // enforcement of the validator-tag greyed-out mechanism (§3.5.1).
        let rows = client
            .query(
                "SELECT name
                 FROM reborn_tools
                 WHERE tenant_id   = $1
                   AND user_id     = $2
                   AND agent_id    = $3
                   AND project_id  = $4
                   AND class_code  = 0
                   AND validation_status = 'validated'
                   AND NOT ('05:validator' = ANY(consumer_tags))
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
