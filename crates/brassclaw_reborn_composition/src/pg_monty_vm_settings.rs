//! `PgMontyVmSettingsStore` — Postgres-backed Monty VM settings.
//!
//! Reads and writes the `reborn_monty_vm_settings` table (V034 migration).
//! One row per `(tenant_id, user_id, agent_id, project_id)` scope; upsert
//! on write, return compiled-in defaults when no row exists.

#[cfg(feature = "postgres")]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_pg::PgPool;
    use brassclaw_product_workflow::{
        MontyVmSettings, MontyVmSettingsError, MontyVmSettingsStore, UpdateMontyVmSettingsRequest,
        default_monty_vm_settings,
    };
    use tracing::debug;

    /// Postgres-backed implementation of [`MontyVmSettingsStore`].
    #[derive(Clone)]
    pub(crate) struct PgMontyVmSettingsStore {
        pool: Arc<PgPool>,
        tenant_id: String,
        agent_id: String,
    }

    impl PgMontyVmSettingsStore {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            tenant_id: impl Into<String>,
            agent_id: impl Into<String>,
        ) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
                agent_id: agent_id.into(),
            }
        }

        fn map_pool(e: deadpool_postgres::PoolError) -> MontyVmSettingsError {
            MontyVmSettingsError::Unavailable(e.to_string())
        }

        fn map_pg(e: tokio_postgres::Error) -> MontyVmSettingsError {
            MontyVmSettingsError::Internal(e.to_string())
        }
    }

    #[async_trait]
    impl MontyVmSettingsStore for PgMontyVmSettingsStore {
        async fn get(
            &self,
            user_id: &str,
            project_id: &str,
        ) -> Result<MontyVmSettings, MontyVmSettingsError> {
            let client = self.pool.get().await.map_err(Self::map_pool)?;
            let row = client
                .query_opt(
                    "SELECT max_duration_secs, max_allocations, max_memory_bytes,
                            failure_rollback_threshold, active_orchestrator_id,
                            prior_knowledge_token_budget, q4_retention_days,
                            forensic_packet_retention_days
                     FROM reborn_monty_vm_settings
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4",
                    &[
                        &self.tenant_id.as_str(),
                        &user_id,
                        &self.agent_id.as_str(),
                        &project_id,
                    ],
                )
                .await
                .map_err(Self::map_pg)?;

            match row {
                Some(r) => {
                    let max_duration_secs: i32 = r.get(0);
                    let max_allocations: i64 = r.get(1);
                    let max_memory_bytes: i64 = r.get(2);
                    let failure_rollback_threshold: i16 = r.get(3);
                    let active_orchestrator_id: Option<uuid::Uuid> = r.get(4);
                    let prior_knowledge_token_budget: i32 = r.get(5);
                    let q4_retention_days: i32 = r.get(6);
                    let forensic_packet_retention_days: i32 = r.get(7);
                    Ok(MontyVmSettings {
                        max_duration_secs: max_duration_secs.max(0) as u64,
                        max_allocations: Some(max_allocations.max(0) as u64),
                        max_memory_bytes: Some(max_memory_bytes.max(0) as u64),
                        failure_rollback_threshold: failure_rollback_threshold.max(1) as u32,
                        prior_knowledge_token_budget: prior_knowledge_token_budget.max(0) as u32,
                        q4_retention_days: q4_retention_days.max(0) as u32,
                        forensic_packet_retention_days: forensic_packet_retention_days.max(0)
                            as u32,
                        active_orchestrator_id: active_orchestrator_id.map(|u| u.to_string()),
                    })
                }
                None => {
                    debug!(
                        user_id,
                        project_id, "monty_vm_settings: no row found, returning defaults"
                    );
                    Ok(default_monty_vm_settings())
                }
            }
        }

        async fn upsert(
            &self,
            user_id: &str,
            project_id: &str,
            update: &UpdateMontyVmSettingsRequest,
        ) -> Result<MontyVmSettings, MontyVmSettingsError> {
            // Validate active_orchestrator_id if provided — must be a parseable UUID.
            // Full Validated-status enforcement is done at the API layer.
            if let Some(ref id) = update.active_orchestrator_id
                && uuid::Uuid::parse_str(id).is_err()
            {
                return Err(MontyVmSettingsError::Invalid(format!(
                    "active_orchestrator_id is not a valid UUID: {id}"
                )));
            }

            // Load current row first (to fill in unchanged fields).
            let current = self.get(user_id, project_id).await?;

            let max_duration_secs = update
                .max_duration_secs
                .unwrap_or(current.max_duration_secs) as i32;
            let max_allocations = update
                .max_allocations
                .or(current.max_allocations)
                .unwrap_or(5_000_000) as i64;
            let max_memory_bytes = update
                .max_memory_bytes
                .or(current.max_memory_bytes)
                .unwrap_or(128 * 1024 * 1024) as i64;
            let failure_rollback_threshold = update
                .failure_rollback_threshold
                .unwrap_or(current.failure_rollback_threshold)
                as i16;
            let prior_knowledge_token_budget = update
                .prior_knowledge_token_budget
                .unwrap_or(current.prior_knowledge_token_budget)
                as i32;
            let q4_retention_days = update
                .q4_retention_days
                .unwrap_or(current.q4_retention_days) as i32;
            let forensic_packet_retention_days = update
                .forensic_packet_retention_days
                .unwrap_or(current.forensic_packet_retention_days)
                as i32;
            let active_orchestrator_id: Option<uuid::Uuid> = update
                .active_orchestrator_id
                .as_deref()
                .or(current.active_orchestrator_id.as_deref())
                .and_then(|s| uuid::Uuid::parse_str(s).ok());

            let client = self.pool.get().await.map_err(Self::map_pool)?;
            client
                .execute(
                    "INSERT INTO reborn_monty_vm_settings
                         (tenant_id, user_id, agent_id, project_id,
                          max_duration_secs, max_allocations, max_memory_bytes,
                          failure_rollback_threshold, active_orchestrator_id,
                          prior_knowledge_token_budget, q4_retention_days,
                          forensic_packet_retention_days)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
                     ON CONFLICT ON CONSTRAINT reborn_monty_vm_settings_scope_unique
                     DO UPDATE SET
                         max_duration_secs            = EXCLUDED.max_duration_secs,
                         max_allocations              = EXCLUDED.max_allocations,
                         max_memory_bytes             = EXCLUDED.max_memory_bytes,
                         failure_rollback_threshold   = EXCLUDED.failure_rollback_threshold,
                         active_orchestrator_id       = EXCLUDED.active_orchestrator_id,
                         prior_knowledge_token_budget = EXCLUDED.prior_knowledge_token_budget,
                         q4_retention_days            = EXCLUDED.q4_retention_days,
                         forensic_packet_retention_days = EXCLUDED.forensic_packet_retention_days,
                         updated_at                   = now()",
                    &[
                        &self.tenant_id.as_str(),
                        &user_id,
                        &self.agent_id.as_str(),
                        &project_id,
                        &max_duration_secs,
                        &max_allocations,
                        &max_memory_bytes,
                        &failure_rollback_threshold,
                        &active_orchestrator_id,
                        &prior_knowledge_token_budget,
                        &q4_retention_days,
                        &forensic_packet_retention_days,
                    ],
                )
                .await
                .map_err(Self::map_pg)?;

            // Return the row after upsert (re-read to get DB-coerced values).
            self.get(user_id, project_id).await
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgMontyVmSettingsStore;
