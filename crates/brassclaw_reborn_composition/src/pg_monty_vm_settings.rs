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

        #[cfg(feature = "skills-db")]
        pub(crate) async fn settled_get(&self) -> Result<MontyVmSettings, MontyVmSettingsError> {
            let mut client = self.pool.get().await.map_err(Self::map_pool)?;
            let transaction = client.transaction().await.map_err(Self::map_pg)?;
            let row = transaction
                .query_one(
                    "SELECT max_duration_secs, retired_max_allocations, max_memory_bytes,
                 failure_rollback_threshold, active_orchestrator_id, prior_knowledge_token_budget,
                 q4_retention_days, forensic_packet_retention_days, token_budgets_enabled,
                 revision, execution_limits, memory_policy, max_recipes_per_task FROM reborn_monty_vm_settings
                 WHERE tenant_id=$1 AND user_id='default' AND agent_id=$2 AND project_id='default'
                 FOR UPDATE",
                    &[&self.tenant_id, &self.agent_id],
                )
                .await
                .map_err(Self::map_pg)?;
            let result = settings_from_row(&row)?;
            transaction.commit().await.map_err(Self::map_pg)?;
            Ok(result)
        }

        fn map_pool(e: deadpool_postgres::PoolError) -> MontyVmSettingsError {
            MontyVmSettingsError::Unavailable(e.to_string())
        }

        fn map_pg(e: tokio_postgres::Error) -> MontyVmSettingsError {
            MontyVmSettingsError::Internal(e.to_string())
        }
    }

    fn checked_i32(value: Option<u64>, field: &str) -> Result<Option<i32>, MontyVmSettingsError> {
        value
            .map(|value| {
                i32::try_from(value)
                    .map_err(|_| MontyVmSettingsError::Invalid(format!("{field} is out of range")))
            })
            .transpose()
    }

    fn checked_positive_i64(
        value: Option<u64>,
        field: &str,
    ) -> Result<Option<i64>, MontyVmSettingsError> {
        value
            .map(|value| {
                i64::try_from(value)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| {
                        MontyVmSettingsError::Invalid(format!(
                            "{field} must be a positive signed 64-bit integer"
                        ))
                    })
            })
            .transpose()
    }

    fn settings_from_row(
        row: &tokio_postgres::Row,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        let execution_limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(10)).map_err(|_| {
                MontyVmSettingsError::Internal("invalid stored execution limits".into())
            })?;
        execution_limits
            .validate()
            .map_err(|reason| MontyVmSettingsError::Internal(reason.into()))?;
        let memory_policy: brassclaw_product_workflow::MontyMemoryPolicy =
            serde_json::from_value(row.get(11)).map_err(|_| {
                MontyVmSettingsError::Internal("invalid stored memory policy".into())
            })?;
        memory_policy
            .validate()
            .map_err(|reason| MontyVmSettingsError::Internal(reason.into()))?;
        let memory_bytes: i64 = row.get(2);
        if memory_bytes < brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES as i64 {
            return Err(MontyVmSettingsError::Internal(
                "stored heap budget is below the 512 MiB default".into(),
            ));
        }
        // Schema constraints guarantee these signed values are nonnegative.
        Ok(MontyVmSettings {
            max_recipes_per_task: row.get::<_, i32>(12) as u32,
            memory_policy,
            execution_limits,
            revision: row.get::<_, i64>(9) as u64,
            max_duration_secs: row.get::<_, i32>(0) as u64,
            max_allocations: None,
            allocation_count_limit_supported: false,
            retired_max_allocations: row.get::<_, Option<i64>>(1).map(|value| value as u64),
            max_memory_bytes: Some(memory_bytes as u64),
            failure_rollback_threshold: row.get::<_, i16>(3) as u32,
            active_orchestrator_id: row.get::<_, Option<uuid::Uuid>>(4).map(|id| id.to_string()),
            prior_knowledge_token_budget: row.get::<_, i32>(5) as u32,
            q4_retention_days: row.get::<_, i32>(6) as u32,
            forensic_packet_retention_days: row.get::<_, i32>(7) as u32,
            token_budgets_enabled: row.get(8),
        })
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
                    "SELECT max_duration_secs, retired_max_allocations, max_memory_bytes,
                            failure_rollback_threshold, active_orchestrator_id,
                            prior_knowledge_token_budget, q4_retention_days,
                            forensic_packet_retention_days, token_budgets_enabled, revision, execution_limits, memory_policy, max_recipes_per_task
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
                Some(r) => settings_from_row(&r),
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
            // No read/merge/write cycle: PATCH and revision advance share the
            // row lock in one transaction, including concurrent first writes.
            let expected_revision = update.expected_revision.ok_or_else(|| {
                MontyVmSettingsError::Invalid("expected_revision is required".into())
            })?;
            let expected_revision = i64::try_from(expected_revision).map_err(|_| {
                MontyVmSettingsError::Invalid("expected_revision is out of range".into())
            })?;
            if expected_revision == i64::MAX {
                return Err(MontyVmSettingsError::Invalid(
                    "revision is exhausted".into(),
                ));
            }
            let execution_limits = update
                .execution_limits
                .map(|limits| {
                    limits
                        .validate()
                        .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
                    serde_json::to_value(limits).map_err(|_| {
                        MontyVmSettingsError::Internal("execution limits encoding failed".into())
                    })
                })
                .transpose()?;
            let memory_policy = update
                .memory_policy
                .map(|policy| {
                    policy
                        .validate()
                        .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
                    serde_json::to_value(policy).map_err(|_| {
                        MontyVmSettingsError::Internal("memory policy encoding failed".into())
                    })
                })
                .transpose()?;
            let recipes = checked_i32(
                update.max_recipes_per_task.map(u64::from),
                "max_recipes_per_task",
            )?;
            if recipes.is_some_and(|value| value <= 0) {
                return Err(MontyVmSettingsError::Invalid(
                    "max_recipes_per_task must be a positive signed 32-bit integer".into(),
                ));
            }
            let duration = checked_i32(update.max_duration_secs, "max_duration_secs")?;
            if duration.is_some_and(|value| value <= 0) {
                return Err(MontyVmSettingsError::Invalid(
                    "max_duration_secs must be a positive signed 32-bit integer".into(),
                ));
            }
            if update.max_allocations.is_some() {
                return Err(MontyVmSettingsError::Invalid(
                    "max_allocations was removed by Monty 1.0; historical values are read-only"
                        .into(),
                ));
            }
            let memory = checked_positive_i64(update.max_memory_bytes, "max_memory_bytes")?;
            if memory.is_some_and(|bytes| {
                bytes < brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES as i64
            }) {
                return Err(MontyVmSettingsError::Invalid(
                    "max_memory_bytes must be at least the 512 MiB default".into(),
                ));
            }
            let threshold = update
                .failure_rollback_threshold
                .map(|value| {
                    i16::try_from(value)
                        .ok()
                        .filter(|value| *value > 0)
                        .ok_or_else(|| {
                            MontyVmSettingsError::Invalid(
                                "failure_rollback_threshold is out of range".into(),
                            )
                        })
                })
                .transpose()?;
            let prior_budget = checked_i32(
                update.prior_knowledge_token_budget.map(u64::from),
                "prior_knowledge_token_budget",
            )?;
            let retention =
                checked_i32(update.q4_retention_days.map(u64::from), "q4_retention_days")?;
            if prior_budget == Some(0) || retention == Some(0) {
                return Err(MontyVmSettingsError::Invalid(
                    "token budget and Q4 retention must be positive".into(),
                ));
            }
            let forensic_retention = checked_i32(
                update.forensic_packet_retention_days.map(u64::from),
                "forensic_packet_retention_days",
            )?;
            let orchestrator = update
                .active_orchestrator_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| {
                    MontyVmSettingsError::Invalid("active_orchestrator_id is not a UUID".into())
                })?;
            let mut client = self.pool.get().await.map_err(Self::map_pool)?;
            let transaction = client.transaction().await.map_err(Self::map_pg)?;
            // Defaults match the API for first write; existing rows retain all
            // fields not present in the patch. Failed CAS rolls back insertion.
            let defaults = default_monty_vm_settings();
            let default_budget = defaults.prior_knowledge_token_budget as i32;
            transaction
                .execute(
                    "INSERT INTO reborn_monty_vm_settings
                 (tenant_id, user_id, agent_id, project_id, prior_knowledge_token_budget)
                 VALUES ($1,$2,$3,$4,$5)
                 ON CONFLICT ON CONSTRAINT reborn_monty_vm_settings_scope_unique DO NOTHING",
                    &[
                        &self.tenant_id,
                        &user_id,
                        &self.agent_id,
                        &project_id,
                        &default_budget,
                    ],
                )
                .await
                .map_err(Self::map_pg)?;
            let row = transaction
                .query_opt(
                    "UPDATE reborn_monty_vm_settings SET
                 max_duration_secs = COALESCE($5, max_duration_secs),
                 max_memory_bytes = COALESCE($6, max_memory_bytes),
                 failure_rollback_threshold = COALESCE($7, failure_rollback_threshold),
                 active_orchestrator_id = COALESCE($8, active_orchestrator_id),
                 prior_knowledge_token_budget = COALESCE($9, prior_knowledge_token_budget),
                 q4_retention_days = COALESCE($10, q4_retention_days),
                 forensic_packet_retention_days = COALESCE($11, forensic_packet_retention_days),
                 token_budgets_enabled = COALESCE($12, token_budgets_enabled),
                 execution_limits = COALESCE($14, execution_limits),
                 memory_policy = COALESCE($15, memory_policy),
                 max_recipes_per_task = COALESCE($16, max_recipes_per_task),
                 revision = revision + 1
                 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                   AND revision=$13
                 RETURNING max_duration_secs, retired_max_allocations, max_memory_bytes,
                   failure_rollback_threshold, active_orchestrator_id,
                   prior_knowledge_token_budget, q4_retention_days,
                   forensic_packet_retention_days, token_budgets_enabled, revision, execution_limits, memory_policy, max_recipes_per_task",
                    &[
                        &self.tenant_id,
                        &user_id,
                        &self.agent_id,
                        &project_id,
                        &duration,
                        &memory,
                        &threshold,
                        &orchestrator,
                        &prior_budget,
                        &retention,
                        &forensic_retention,
                        &update.token_budgets_enabled,
                        &expected_revision,
                        &execution_limits,
                        &memory_policy,
                        &recipes,
                    ],
                )
                .await
                .map_err(Self::map_pg)?;
            let Some(row) = row else {
                transaction.rollback().await.map_err(Self::map_pg)?;
                return Err(MontyVmSettingsError::RevisionConflict);
            };
            let settings = settings_from_row(&row)?;
            transaction.commit().await.map_err(Self::map_pg)?;
            // Return this transaction's generation, not a later concurrent edit.
            Ok(settings)
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgMontyVmSettingsStore;

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use std::sync::Arc;

    use brassclaw_product_workflow::{
        MontyVmSettingsError, MontyVmSettingsStore, UpdateMontyVmSettingsRequest,
    };

    use super::PgMontyVmSettingsStore;

    #[tokio::test]
    async fn native_removed_allocation_limit_preserves_evidence_and_rejects_writes() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        // V034's actual seed went through the complete production bundle.
        let seed = client.query_one("SELECT retired_max_allocations,revision FROM reborn_monty_vm_settings WHERE tenant_id='__system__'", &[]).await.unwrap();
        assert_eq!(seed.get::<_, Option<i64>>(0), Some(5_000_000));
        let seed_revision: i64 = seed.get(1);
        let store = PgMontyVmSettingsStore::new(rig.pool.clone(), "__system__", "__system__");
        let settings = store.get("__system__", "__system__").await.unwrap();
        assert_eq!(settings.max_allocations, None);
        assert!(!settings.allocation_count_limit_supported);
        assert_eq!(settings.retired_max_allocations, Some(5_000_000));
        assert_eq!(settings.revision, seed_revision as u64);
        let update = serde_json::from_value(
            serde_json::json!({"expected_revision": settings.revision, "max_allocations": 7}),
        )
        .unwrap();
        assert!(matches!(
            store.upsert("__system__", "__system__", &update).await,
            Err(MontyVmSettingsError::Invalid(_))
        ));
        assert_eq!(
            store
                .get("__system__", "__system__")
                .await
                .unwrap()
                .revision,
            settings.revision
        );

        // Reapply the exact migration to a historical full table shape, with
        // an explicitly edited count and an exhausted settings generation.
        client.batch_execute("CREATE SCHEMA legacy_monty_test;
            CREATE TABLE legacy_monty_test.reborn_monty_vm_settings (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO legacy_monty_test, public;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings RENAME COLUMN retired_max_allocations TO max_allocations;
            ALTER TABLE reborn_monty_vm_settings ALTER COLUMN max_allocations SET NOT NULL;
            ALTER TABLE reborn_monty_vm_settings ALTER COLUMN max_allocations SET DEFAULT 5000000;
            INSERT INTO reborn_monty_vm_settings (tenant_id,user_id,agent_id,project_id,max_allocations,revision,max_duration_secs)
                VALUES ('legacy','operator','agent','project',1234567,9223372036854775807,900);").await.unwrap();
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V100__retire_monty_allocation_count.sql"
            ))
            .await
            .unwrap();
        let preserved = client.query_one("SELECT retired_max_allocations,revision,max_duration_secs FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(preserved.get::<_, Option<i64>>(0), Some(1_234_567));
        assert_eq!(preserved.get::<_, i64>(1), i64::MAX);
        assert_eq!(preserved.get::<_, i32>(2), 900);
        for sql in [
            "UPDATE reborn_monty_vm_settings SET retired_max_allocations=42 WHERE tenant_id='legacy'",
            "INSERT INTO reborn_monty_vm_settings (tenant_id,user_id,agent_id,project_id,retired_max_allocations) VALUES ('new','operator','agent','project',42)",
        ] {
            assert_eq!(
                client.execute(sql, &[]).await.unwrap_err().code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        client.execute("INSERT INTO reborn_monty_vm_settings (tenant_id,user_id,agent_id,project_id) VALUES ('new','operator','agent','project')", &[]).await.unwrap();
        assert_eq!(client.query_one("SELECT retired_max_allocations FROM reborn_monty_vm_settings WHERE tenant_id='new'", &[]).await.unwrap().get::<_, Option<i64>>(0), None);
        assert!(
            client
                .execute("UPDATE reborn_monty_vm_settings SET max_allocations=7", &[])
                .await
                .is_err(),
            "old writers must fail explicitly"
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_memory_policy_upgrade_preserves_limits_and_new_rows_size_once() {
        use brassclaw_product_workflow::MontyMemoryMode;
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        // Actual pre-upgrade schema shape, isolated from the fixture's full bundle.
        client.batch_execute("CREATE SCHEMA memory_upgrade;
            CREATE TABLE memory_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO memory_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_memory_default_floor;
            ALTER TABLE reborn_monty_vm_settings DROP COLUMN memory_policy;
            ALTER TABLE reborn_monty_vm_settings ALTER COLUMN max_memory_bytes SET DEFAULT 134217728;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,max_memory_bytes,revision)
            VALUES('old','owner','agent','project',80740352,8);").await.unwrap();
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V111__monty_memory_policy.sql"
            ))
            .await
            .unwrap();
        let old = client.query_one("SELECT max_memory_bytes,revision,memory_policy FROM reborn_monty_vm_settings WHERE tenant_id='old'", &[]).await.unwrap();
        assert_eq!(old.get::<_, i64>(0), 77 * 1024 * 1024);
        assert_eq!(old.get::<_, i64>(1), 9);
        let policy: brassclaw_product_workflow::MontyMemoryPolicy =
            serde_json::from_value(old.get(2)).unwrap();
        assert_eq!(policy.mode, MontyMemoryMode::Manual);
        let new = client
            .query_one(
                "INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id)
            VALUES('new','owner','agent','project') RETURNING max_memory_bytes,memory_policy",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(new.get::<_, i64>(0), 512 * 1024 * 1024);
        let policy: brassclaw_product_workflow::MontyMemoryPolicy =
            serde_json::from_value(new.get(1)).unwrap();
        assert_eq!(policy.mode, MontyMemoryMode::Startup);
        assert_eq!(policy.sample_interval_secs, 60);
        policy.validate().unwrap();
        // The new operator floor intentionally supersedes V111's preservation
        // of smaller limits, while retaining larger limits and their revisions.
        client.execute("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,max_memory_bytes,revision) VALUES('large','owner','agent','project',1073741824,12)", &[]).await.unwrap();
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V114__monty_memory_default_floor.sql"
            ))
            .await
            .unwrap();
        let old = client.query_one("SELECT max_memory_bytes,revision,memory_policy FROM reborn_monty_vm_settings WHERE tenant_id='old'", &[]).await.unwrap();
        assert_eq!(
            old.get::<_, i64>(0),
            brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES as i64
        );
        assert_eq!(old.get::<_, i64>(1), 10);
        let policy: brassclaw_product_workflow::MontyMemoryPolicy =
            serde_json::from_value(old.get(2)).unwrap();
        assert_eq!(policy.mode, MontyMemoryMode::Manual);
        let large = client.query_one("SELECT max_memory_bytes,revision FROM reborn_monty_vm_settings WHERE tenant_id='large'", &[]).await.unwrap();
        assert_eq!(large.get::<_, i64>(0), 1024 * 1024 * 1024);
        assert_eq!(large.get::<_, i64>(1), 12);
        assert_eq!(client.execute("UPDATE reborn_monty_vm_settings SET max_memory_bytes=536870911 WHERE tenant_id='old'", &[]).await.unwrap_err().code(), Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION));
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_duration_upgrade_preserves_values_and_allows_positive_int_range() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA duration_upgrade;
            CREATE TABLE duration_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO duration_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT reborn_monty_vm_settings_max_duration_secs_check;
            ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT reborn_monty_vm_settings_max_duration_secs_check
                CHECK(max_duration_secs BETWEEN 30 AND 3600);
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,max_duration_secs,revision)
            VALUES('old','owner','agent','project',900,8);").await.unwrap();
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V112__monty_task_duration_range.sql"
            ))
            .await
            .unwrap();
        let old = client.query_one("SELECT max_duration_secs,revision FROM reborn_monty_vm_settings WHERE tenant_id='old'", &[]).await.unwrap();
        assert_eq!(old.get::<_, i32>(0), 900);
        assert_eq!(old.get::<_, i64>(1), 8);
        for duration in [1, 7200, i32::MAX] {
            let actual = client
                .query_one(
                    "INSERT INTO reborn_monty_vm_settings
                (tenant_id,user_id,agent_id,project_id,max_duration_secs)
                VALUES($1,'owner','agent','project',$2) RETURNING max_duration_secs",
                    &[&format!("duration-{duration}"), &duration],
                )
                .await
                .unwrap();
            assert_eq!(actual.get::<_, i32>(0), duration);
        }
        for duration in [0, -1] {
            let error = client
                .execute(
                    "INSERT INTO reborn_monty_vm_settings
                (tenant_id,user_id,agent_id,project_id,max_duration_secs)
                VALUES($1,'owner','agent','project',$2)",
                    &[&format!("invalid-{duration}"), &duration],
                )
                .await
                .unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_recipe_capacity_upgrade_preserves_settings_and_revision() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA recipe_capacity_upgrade;
            CREATE TABLE recipe_capacity_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO recipe_capacity_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP COLUMN max_recipes_per_task;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,max_duration_secs,revision)
            VALUES('old','owner','agent','project',900,8);").await.unwrap();
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V113__monty_recipe_selection_capacity.sql"
            ))
            .await
            .unwrap();
        let old = client.query_one("SELECT max_recipes_per_task,max_duration_secs,revision FROM reborn_monty_vm_settings WHERE tenant_id='old'", &[]).await.unwrap();
        assert_eq!(old.get::<_, i32>(0), 8);
        assert_eq!(old.get::<_, i32>(1), 900);
        assert_eq!(old.get::<_, i64>(2), 8);
        for capacity in [1, 16, i32::MAX] {
            let actual = client.query_one("UPDATE reborn_monty_vm_settings SET max_recipes_per_task=$1 WHERE tenant_id='old' RETURNING max_recipes_per_task", &[&capacity]).await.unwrap();
            assert_eq!(actual.get::<_, i32>(0), capacity);
        }
        for capacity in [0, -1] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET max_recipes_per_task=$1 WHERE tenant_id='old'", &[&capacity]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_child_context_upgrade_pins_default_and_preserves_explicit_values() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA child_capacity_upgrade;
            CREATE TABLE child_capacity_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO child_capacity_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_actor_transport_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_attempt_retention_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_child_context_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_admitted_backlog;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_publication_capacity;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=(execution_limits-'max_recipe_contexts')||'{\"max_feeds\":777}'::jsonb WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"max_recipe_contexts\":128}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old_limits: serde_json::Value = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        let legacy: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old_limits).unwrap();
        assert_eq!(legacy.max_recipe_contexts, 64);
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V115__monty_child_context_capacity.sql"
            ))
            .await
            .unwrap();
        let legacy = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(legacy.get(0)).unwrap();
        assert_eq!(limits.max_recipe_contexts, 64);
        assert_eq!(limits.max_feeds, 777);
        assert_eq!(legacy.get::<_, i64>(1), 9);
        let explicit = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='explicit'", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(explicit.get(0)).unwrap();
        assert_eq!(limits.max_recipe_contexts, 128);
        assert_eq!(explicit.get::<_, i64>(1), 12);
        for value in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(u32::MAX as u64 + 1),
            serde_json::json!("2"),
            serde_json::json!(null),
            serde_json::json!(1.5),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_recipe_contexts}',$1) WHERE tenant_id='explicit'", &[&value]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.max_recipe_contexts, 64);
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_backlog_upgrade_preserves_explicit_limits_and_rejects_invalid_numbers() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA backlog_upgrade;
            CREATE TABLE backlog_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO backlog_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_actor_transport_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_attempt_retention_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_admitted_backlog;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_publication_capacity;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('partial','owner','agent','project',10),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_queued_tasks'-'max_queued_bytes' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=(execution_limits-'max_queued_bytes')||'{\"max_queued_tasks\":128}'::jsonb WHERE tenant_id='partial';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"max_queued_tasks\":128,\"max_queued_bytes\":134217728}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.max_queued_tasks, 64);
        assert_eq!(decoded.max_queued_bytes, 64 * 1024 * 1024);
        let migration =
            include_str!("../../brassclaw_pg/migrations/V116__monty_admitted_backlog.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, count, bytes, revision) in [
            ("legacy", 64, 67108864, 9i64),
            ("partial", 128, 67108864, 11),
            ("explicit", 128, 134217728, 12),
        ] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.max_queued_tasks, count);
            assert_eq!(limits.max_queued_bytes, bytes);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for field in ["max_queued_tasks", "max_queued_bytes"] {
            for value in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!("2"),
                serde_json::json!(null),
                serde_json::json!(1.5),
            ] {
                let path = vec![field.to_owned()];
                let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,$1,$2) WHERE tenant_id='explicit'", &[&path, &value]).await.unwrap_err();
                assert_eq!(
                    error.code(),
                    Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
                );
            }
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-$1 WHERE tenant_id='explicit'", &[&field]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let overflow = serde_json::json!(u32::MAX as u64 + 1);
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_queued_tasks}',$1) WHERE tenant_id='explicit'", &[&overflow]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.max_queued_tasks, 64);
        assert_eq!(limits.max_queued_bytes, 67108864);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_admitted_backlog;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_queued_tasks',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("max_queued_tasks")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_settings_capacity_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA settings_capacity_upgrade;
            CREATE TABLE settings_capacity_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO settings_capacity_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_actor_transport_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_attempt_retention_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_publication_capacity;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_pending_settings' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"max_pending_settings\":32}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let migration = include_str!(
            "../../brassclaw_pg/migrations/V117__monty_settings_publication_capacity.sql"
        );
        client.batch_execute(migration).await.unwrap();
        for (tenant, capacity, revision) in [("legacy", 8, 9i64), ("explicit", 32, 12)] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.max_pending_settings, capacity);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for value in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(u32::MAX as u64 + 1),
            serde_json::json!("2"),
            serde_json::json!(null),
            serde_json::json!(1.5),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_pending_settings}',$1) WHERE tenant_id='explicit'", &[&value]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_pending_settings' WHERE tenant_id='explicit'", &[]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let maximum = serde_json::json!(u32::MAX);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_pending_settings}',$1) WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.max_pending_settings, 8);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_publication_capacity;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_pending_settings',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("max_pending_settings")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_settings_deadline_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA settings_deadline_upgrade;
            CREATE TABLE settings_deadline_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO settings_deadline_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('partial','owner','agent','project',12),('explicit','owner','agent','project',16);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-
                ARRAY['settings_source_timeout_millis','settings_uptake_timeout_millis'] WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits-
                'settings_source_timeout_millis','{settings_uptake_timeout_millis}','9000') WHERE tenant_id='partial';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||
                '{\"settings_source_timeout_millis\":7000,\"settings_uptake_timeout_millis\":9000}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.settings_source_timeout_millis, 2000);
        assert_eq!(decoded.settings_uptake_timeout_millis, 5000);
        let migration = include_str!(
            "../../brassclaw_pg/migrations/V121__monty_settings_coordination_deadlines.sql"
        );
        client.batch_execute(migration).await.unwrap();
        for (tenant, startup, response, revision) in [
            ("legacy", 2000, 5000, 9i64),
            ("partial", 2000, 9000, 13),
            ("explicit", 7000, 9000, 16),
        ] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.settings_source_timeout_millis, startup);
            assert_eq!(limits.settings_uptake_timeout_millis, response);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for key in [
            "settings_source_timeout_millis",
            "settings_uptake_timeout_millis",
        ] {
            for value in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!("2"),
                serde_json::json!(null),
                serde_json::json!(true),
                serde_json::json!(1.5),
                serde_json::from_str("18446744073709551616").unwrap(),
            ] {
                let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,ARRAY[$1::text],$2) WHERE tenant_id='explicit'", &[&key, &value]).await.unwrap_err();
                assert_eq!(
                    error.code(),
                    Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
                );
            }
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-$1::text WHERE tenant_id='explicit'", &[&key]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.settings_source_timeout_millis, 2000);
        assert_eq!(limits.settings_uptake_timeout_millis, 5000);
        let maximum = serde_json::json!({"settings_source_timeout_millis":u64::MAX,"settings_uptake_timeout_millis":u64::MAX});
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||$1 WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.settings_uptake_timeout_millis, u64::MAX);
        // Clock representations differ across platforms; never wrap/truncate a
        // deadline or impose a fabricated common clock maximum.
        #[cfg(feature = "skills-db")]
        {
            // Native VM consumption is part of the skills-db product; SQL and
            // DTO representation checks above also run in the PG-only build.
            let native_supported = std::time::Instant::now()
                .checked_add(std::time::Duration::from_millis(u64::MAX))
                .is_some();
            assert_eq!(
                crate::live_monty_settings::execution_bounds(limits).is_ok(),
                native_supported
            );
        }
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'settings_source_timeout_millis',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("settings_source_timeout_millis")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_ownership_controls_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA ownership_control_upgrade;
            CREATE TABLE ownership_control_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO ownership_control_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_ownership_controls;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('partial','owner','agent','project',12),('explicit','owner','agent','project',16);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-
                ARRAY['ownership_check_timeout_millis','ownership_heartbeat_interval_millis','max_pending_ownership_checks'] WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits-
                'ownership_check_timeout_millis','{ownership_heartbeat_interval_millis}','9000') WHERE tenant_id='partial';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||
                '{\"ownership_check_timeout_millis\":7000,\"ownership_heartbeat_interval_millis\":9000,\"max_pending_ownership_checks\":17}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.ownership_check_timeout_millis, 2000);
        assert_eq!(decoded.ownership_heartbeat_interval_millis, 2000);
        assert_eq!(decoded.max_pending_ownership_checks, 8);
        let migration =
            include_str!("../../brassclaw_pg/migrations/V125__monty_ownership_controls.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, startup, response, revision) in [
            ("legacy", 2000, 2000, 9i64),
            ("partial", 2000, 9000, 13),
            ("explicit", 7000, 9000, 16),
        ] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.ownership_check_timeout_millis, startup);
            assert_eq!(limits.ownership_heartbeat_interval_millis, response);
            assert_eq!(
                limits.max_pending_ownership_checks,
                if tenant == "explicit" { 17 } else { 8 }
            );
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for key in [
            "ownership_check_timeout_millis",
            "ownership_heartbeat_interval_millis",
            "max_pending_ownership_checks",
        ] {
            for value in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!("2"),
                serde_json::json!(null),
                serde_json::json!(true),
                serde_json::json!(1.5),
                serde_json::from_str("18446744073709551616").unwrap(),
            ] {
                let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,ARRAY[$1::text],$2) WHERE tenant_id='explicit'", &[&key, &value]).await.unwrap_err();
                assert_eq!(
                    error.code(),
                    Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
                );
            }
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-$1::text WHERE tenant_id='explicit'", &[&key]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.ownership_check_timeout_millis, 2000);
        assert_eq!(limits.ownership_heartbeat_interval_millis, 2000);
        assert_eq!(limits.max_pending_ownership_checks, 8);
        let overflow = serde_json::json!(u32::MAX as u64 + 1);
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_pending_ownership_checks}',$1) WHERE tenant_id='explicit'", &[&overflow]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let maximum = serde_json::json!({"ownership_check_timeout_millis":u64::MAX,"ownership_heartbeat_interval_millis":u64::MAX,"max_pending_ownership_checks":u32::MAX});
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||$1 WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.ownership_heartbeat_interval_millis, u64::MAX);
        // Clock representations differ across platforms; never wrap/truncate a
        // deadline or impose a fabricated common clock maximum.
        #[cfg(feature = "skills-db")]
        {
            // Native VM consumption is part of the skills-db product; SQL and
            // DTO representation checks above also run in the PG-only build.
            let native_supported = std::time::Instant::now()
                .checked_add(std::time::Duration::from_millis(u64::MAX))
                .is_some();
            assert_eq!(
                crate::live_monty_settings::execution_bounds(limits).is_ok(),
                native_supported
            );
        }
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_ownership_controls;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'ownership_check_timeout_millis',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("ownership_check_timeout_millis")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_adapter_reserve_upgrade_preserves_zero_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA adapter_reserve_upgrade;
            CREATE TABLE adapter_reserve_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO adapter_reserve_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_worker_adapter_reserve;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'worker_adapter_reserve_bytes' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"worker_adapter_reserve_bytes\":0}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(
            decoded.worker_adapter_reserve_bytes,
            brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES
        );
        let migration =
            include_str!("../../brassclaw_pg/migrations/V132__monty_worker_adapter_reserve.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, reserve, revision) in [("legacy", 4194304u64, 9i64), ("explicit", 0, 12)] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.worker_adapter_reserve_bytes, reserve);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(null),
            serde_json::json!("2"),
            serde_json::json!(true),
            serde_json::json!(1.5),
            serde_json::from_str("18446744073709551616").unwrap(),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{worker_adapter_reserve_bytes}',$1) WHERE tenant_id='explicit'", &[&invalid]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'worker_adapter_reserve_bytes' WHERE tenant_id='explicit'", &[]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        // The neutral stored contract admits u64. Native layout validation is
        // stricter and must reject overflow before publishing/persisting live edits.
        let maximum = serde_json::json!(u64::MAX);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{worker_adapter_reserve_bytes}',$1) WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.worker_adapter_reserve_bytes, u64::MAX);
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('fresh','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let fresh: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(fresh, brassclaw_host_api::MontyExecutionLimits::default());
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_worker_adapter_reserve;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'worker_adapter_reserve_bytes',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("worker_adapter_reserve_bytes")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_browser_status_interval_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA browser_status_upgrade;
            CREATE TABLE browser_status_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO browser_status_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_browser_status_interval;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'status_poll_interval_millis' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"status_poll_interval_millis\":9000}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.status_poll_interval_millis, 3000);
        let migration =
            include_str!("../../brassclaw_pg/migrations/V130__monty_browser_status_interval.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, timeout, revision) in [("legacy", 3000, 9i64), ("explicit", 9000, 12)] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.status_poll_interval_millis, timeout);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for value in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(null),
            serde_json::json!("2"),
            serde_json::json!(true),
            serde_json::json!(1.5),
            serde_json::json!(2147483648u64),
            serde_json::from_str("18446744073709551616").unwrap(),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{status_poll_interval_millis}',$1) WHERE tenant_id='explicit'", &[&value]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'status_poll_interval_millis' WHERE tenant_id='explicit'", &[]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let maximum = serde_json::json!(i32::MAX as u32);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{status_poll_interval_millis}',$1) WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.status_poll_interval_millis, i32::MAX as u32);
        let minimum = serde_json::json!(1);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{status_poll_interval_millis}',$1) WHERE tenant_id='explicit'", &[&minimum]).await.unwrap();

        assert!(limits.validate().is_ok());
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let fresh: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(fresh.status_poll_interval_millis, 3000);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_browser_status_interval;
                    UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'status_poll_interval_millis',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("status_poll_interval_millis")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_reconcile_interval_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA reconcile_interval_upgrade;
            CREATE TABLE reconcile_interval_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO reconcile_interval_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_reconcile_interval;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'settings_reconcile_interval_millis' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"settings_reconcile_interval_millis\":9000}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.settings_reconcile_interval_millis, 1000);
        let migration = include_str!(
            "../../brassclaw_pg/migrations/V128__monty_settings_reconcile_interval.sql"
        );
        client.batch_execute(migration).await.unwrap();
        for (tenant, timeout, revision) in [("legacy", 1000, 9i64), ("explicit", 9000, 12)] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.settings_reconcile_interval_millis, timeout);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for value in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(null),
            serde_json::json!("2"),
            serde_json::json!(true),
            serde_json::json!(1.5),
            serde_json::from_str("18446744073709551616").unwrap(),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{settings_reconcile_interval_millis}',$1) WHERE tenant_id='explicit'", &[&value]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'settings_reconcile_interval_millis' WHERE tenant_id='explicit'", &[]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let maximum = serde_json::json!(u64::MAX);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{settings_reconcile_interval_millis}',$1) WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.settings_reconcile_interval_millis, u64::MAX);
        #[cfg(feature = "skills-db")]
        assert_eq!(
            crate::live_monty_settings::execution_bounds(limits).is_ok(),
            std::time::Instant::now()
                .checked_add(std::time::Duration::from_millis(u64::MAX))
                .is_some()
        );
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let fresh: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(fresh.settings_reconcile_interval_millis, 1000);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_reconcile_interval;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'settings_reconcile_interval_millis',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("settings_reconcile_interval_millis")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_cancellation_deadline_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA cancellation_deadline_upgrade;
            CREATE TABLE cancellation_deadline_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO cancellation_deadline_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_cancellation_ack_deadline;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'cancellation_ack_timeout_millis' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"cancellation_ack_timeout_millis\":9000}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.cancellation_ack_timeout_millis, 5000);
        let migration =
            include_str!("../../brassclaw_pg/migrations/V126__monty_cancellation_ack_deadline.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, timeout, revision) in [("legacy", 5000, 9i64), ("explicit", 9000, 12)] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.cancellation_ack_timeout_millis, timeout);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for value in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(null),
            serde_json::json!("2"),
            serde_json::json!(true),
            serde_json::json!(1.5),
            serde_json::from_str("18446744073709551616").unwrap(),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{cancellation_ack_timeout_millis}',$1) WHERE tenant_id='explicit'", &[&value]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'cancellation_ack_timeout_millis' WHERE tenant_id='explicit'", &[]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let maximum = serde_json::json!(u64::MAX);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{cancellation_ack_timeout_millis}',$1) WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.cancellation_ack_timeout_millis, u64::MAX);
        #[cfg(feature = "skills-db")]
        assert_eq!(
            crate::live_monty_settings::execution_bounds(limits).is_ok(),
            std::time::Instant::now()
                .checked_add(std::time::Duration::from_millis(u64::MAX))
                .is_some()
        );
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let fresh: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(fresh.cancellation_ack_timeout_millis, 5000);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_cancellation_ack_deadline;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'cancellation_ack_timeout_millis',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("cancellation_ack_timeout_millis")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_hosting_deadline_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA hosting_deadline_upgrade;
            CREATE TABLE hosting_deadline_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO hosting_deadline_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_hosting_deadlines;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('partial','owner','agent','project',12),('explicit','owner','agent','project',16);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-
                ARRAY['startup_timeout_millis','response_timeout_millis'] WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits-
                'startup_timeout_millis','{response_timeout_millis}','60000') WHERE tenant_id='partial';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||
                '{\"startup_timeout_millis\":45000,\"response_timeout_millis\":60000}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.startup_timeout_millis, 30000);
        assert_eq!(decoded.response_timeout_millis, 30000);
        let migration =
            include_str!("../../brassclaw_pg/migrations/V120__monty_hosting_deadlines.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, startup, response, revision) in [
            ("legacy", 30000, 30000, 9i64),
            ("partial", 30000, 60000, 13),
            ("explicit", 45000, 60000, 16),
        ] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.startup_timeout_millis, startup);
            assert_eq!(limits.response_timeout_millis, response);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for key in ["startup_timeout_millis", "response_timeout_millis"] {
            for value in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!("2"),
                serde_json::json!(null),
                serde_json::json!(true),
                serde_json::json!(1.5),
                serde_json::from_str("18446744073709551616").unwrap(),
            ] {
                let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,ARRAY[$1::text],$2) WHERE tenant_id='explicit'", &[&key, &value]).await.unwrap_err();
                assert_eq!(
                    error.code(),
                    Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
                );
            }
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-$1::text WHERE tenant_id='explicit'", &[&key]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        for patch in [
            serde_json::json!({"startup_timeout_millis":60001}),
            serde_json::json!({"execution_slice_millis":60000}),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||$1 WHERE tenant_id='explicit'", &[&patch]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.startup_timeout_millis, 30000);
        assert_eq!(limits.response_timeout_millis, 30000);
        let maximum = serde_json::json!({"startup_timeout_millis":u64::MAX,"response_timeout_millis":u64::MAX});
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||$1 WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='explicit'",
                &[],
            )
            .await
            .unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.response_timeout_millis, u64::MAX);
        // Clock representations differ across platforms; never wrap/truncate a
        // deadline or impose a fabricated common clock maximum.
        #[cfg(feature = "skills-db")]
        {
            // Native VM consumption is part of the skills-db product; SQL and
            // DTO representation checks above also run in the PG-only build.
            let native_supported = std::time::Instant::now()
                .checked_add(std::time::Duration::from_millis(u64::MAX))
                .is_some();
            assert_eq!(
                crate::live_monty_settings::hosting_deadlines(limits).is_ok(),
                native_supported
            );
        }
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_hosting_deadlines;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'startup_timeout_millis',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("startup_timeout_millis")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_actor_capacity_upgrade_preserves_explicit_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA actor_capacity_upgrade;
            CREATE TABLE actor_capacity_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO actor_capacity_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_actor_transport_capacity;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('partial','owner','agent','project',12),('explicit','owner','agent','project',16);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-
                ARRAY['max_actor_requests','max_actor_reserved_bytes','max_actor_control_requests','max_actor_control_reserved_bytes']
                WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits-
                'max_actor_control_reserved_bytes','{max_actor_requests}','2048') WHERE tenant_id='partial';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.max_actor_requests, 8);
        assert_eq!(decoded.max_actor_control_reserved_bytes, 1073741824);
        let migration =
            include_str!("../../brassclaw_pg/migrations/V119__monty_actor_transport_capacity.sql");
        client.batch_execute(migration).await.unwrap();
        for (tenant, requests, revision) in [
            ("legacy", 8, 9i64),
            ("partial", 2048, 13),
            ("explicit", 8, 16),
        ] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.max_actor_requests, requests);
            assert_eq!(limits.max_actor_control_requests, 8);
            assert_eq!(limits.max_actor_control_reserved_bytes, 1073741824);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for key in [
            "max_actor_requests",
            "max_actor_reserved_bytes",
            "max_actor_control_requests",
            "max_actor_control_reserved_bytes",
        ] {
            for value in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!("2"),
                serde_json::json!(null),
                serde_json::json!(true),
                serde_json::json!(1.5),
            ] {
                let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,ARRAY[$1::text],$2) WHERE tenant_id='explicit'", &[&key, &value]).await.unwrap_err();
                assert_eq!(
                    error.code(),
                    Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
                );
            }
            let maximum = if key.ends_with("bytes") {
                serde_json::json!(u64::MAX)
            } else {
                serde_json::json!(u32::MAX)
            };
            client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,ARRAY[$1::text],$2) WHERE tenant_id='explicit'", &[&key, &maximum]).await.unwrap();
            let overflow: serde_json::Value = if key.ends_with("bytes") {
                serde_json::from_str("18446744073709551616").unwrap()
            } else {
                serde_json::json!(u32::MAX as u64 + 1)
            };
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,ARRAY[$1::text],$2) WHERE tenant_id='explicit'", &[&key, &overflow]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-$1::text WHERE tenant_id='explicit'", &[&key]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.max_actor_requests, 8);
        assert_eq!(limits.max_actor_reserved_bytes, 1073741824);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_actor_transport_capacity;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_actor_requests',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("max_actor_requests")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_attempt_retention_upgrade_preserves_values_and_fails_atomically() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE SCHEMA retention_capacity_upgrade;
            CREATE TABLE retention_capacity_upgrade.reborn_monty_vm_settings
            (LIKE public.reborn_monty_vm_settings INCLUDING ALL);
            SET search_path TO retention_capacity_upgrade;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_worker_adapter_reserve;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_browser_status_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_settings_reconcile_interval;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_cancellation_ack_deadline;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_ownership_controls;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_settings_coordination_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_actor_transport_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_attempt_retention_capacity;
            INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
            VALUES('legacy','owner','agent','project',8),('explicit','owner','agent','project',12);
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_retained_attempts' WHERE tenant_id='legacy';
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"max_retained_attempts\":512}'::jsonb WHERE tenant_id='explicit';").await.unwrap();
        let old = client
            .query_one(
                "SELECT execution_limits FROM reborn_monty_vm_settings WHERE tenant_id='legacy'",
                &[],
            )
            .await
            .unwrap();
        let decoded: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(old.get(0)).unwrap();
        assert_eq!(decoded.max_retained_attempts, 256);
        let migration = include_str!(
            "../../brassclaw_pg/migrations/V118__monty_attempt_retention_capacity.sql"
        );
        client.batch_execute(migration).await.unwrap();
        for (tenant, capacity, revision) in [("legacy", 256, 9i64), ("explicit", 512, 12)] {
            let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id=$1", &[&tenant]).await.unwrap();
            let limits: brassclaw_host_api::MontyExecutionLimits =
                serde_json::from_value(row.get(0)).unwrap();
            assert_eq!(limits.max_retained_attempts, capacity);
            assert_eq!(row.get::<_, i64>(1), revision);
        }
        for value in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(u32::MAX as u64 + 1),
            serde_json::json!("2"),
            serde_json::json!(null),
            serde_json::json!(1.5),
            serde_json::json!(true),
        ] {
            let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_retained_attempts}',$1) WHERE tenant_id='explicit'", &[&value]).await.unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let error = client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_retained_attempts' WHERE tenant_id='explicit'", &[]).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let maximum = serde_json::json!(u32::MAX);
        client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=jsonb_set(execution_limits,'{max_retained_attempts}',$1) WHERE tenant_id='explicit'", &[&maximum]).await.unwrap();
        let row = client.query_one("INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id) VALUES('new','owner','agent','project') RETURNING execution_limits", &[]).await.unwrap();
        let limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(limits.max_retained_attempts, 256);
        client.batch_execute("ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_hosting_deadlines;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT IF EXISTS monty_actor_transport_capacity;
            ALTER TABLE reborn_monty_vm_settings DROP CONSTRAINT monty_attempt_retention_capacity;
            UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits-'max_retained_attempts',revision=9223372036854775807 WHERE tenant_id='legacy';").await.unwrap();
        let error = client.batch_execute(migration).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::NUMERIC_VALUE_OUT_OF_RANGE)
        );
        let row = client.query_one("SELECT execution_limits,revision FROM reborn_monty_vm_settings WHERE tenant_id='legacy'", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(1), i64::MAX);
        assert!(
            row.get::<_, serde_json::Value>(0)
                .get("max_retained_attempts")
                .is_none()
        );
        client.batch_execute("RESET search_path").await.unwrap();
    }

    #[tokio::test]
    async fn native_monty_settings_patch_is_revision_checked_and_atomic() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let store = PgMontyVmSettingsStore::new(Arc::clone(&rig.pool), "settings-cas", "agent");
        let duration: UpdateMontyVmSettingsRequest = serde_json::from_value(serde_json::json!({
            "expected_revision": 0, "max_duration_secs": 900,"max_recipes_per_task":16
        }))
        .unwrap();
        let tokens: UpdateMontyVmSettingsRequest = serde_json::from_value(serde_json::json!({
            "expected_revision": 0, "token_budgets_enabled": true
        }))
        .unwrap();
        let (left, right) = tokio::join!(
            store.upsert("operator", "project", &duration),
            store.upsert("operator", "project", &tokens)
        );
        assert_ne!(left.is_ok(), right.is_ok());
        let (first, mut retry) = match (left, right) {
            (Ok(settings), Err(MontyVmSettingsError::RevisionConflict)) => (settings, tokens),
            (Err(MontyVmSettingsError::RevisionConflict), Ok(settings)) => (settings, duration),
            other => panic!("unexpected concurrent edit results: {other:?}"),
        };
        assert_eq!(first.revision, 1);
        // Rebase only the losing patch. The successful edit must survive.
        retry.expected_revision = Some(first.revision);
        let second = store.upsert("operator", "project", &retry).await.unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(second.max_duration_secs, 900);
        assert_eq!(second.max_recipes_per_task, 16);
        assert!(second.token_budgets_enabled);
        assert_eq!(second.prior_knowledge_token_budget, 100_000);
        assert!(matches!(
            store.upsert("operator", "project", &retry).await,
            Err(MontyVmSettingsError::RevisionConflict)
        ));
        let after = store.get("operator", "project").await.unwrap();
        assert_eq!(after.revision, second.revision);
        assert_eq!(after.max_duration_secs, second.max_duration_secs);
        assert_eq!(after.max_recipes_per_task, second.max_recipes_per_task);
        assert_eq!(
            store
                .get("another-operator", "project")
                .await
                .unwrap()
                .revision,
            0
        );

        for invalid in [
            serde_json::json!({"max_duration_secs": 600}),
            serde_json::json!({"expected_revision": 0, "max_duration_secs": u64::MAX}),
            serde_json::json!({"expected_revision": 0, "max_duration_secs": 0}),
            serde_json::json!({"expected_revision": 0, "max_duration_secs": i32::MAX as u64 + 1}),
            serde_json::json!({"expected_revision": 0, "max_allocations": u64::MAX}),
            serde_json::json!({"expected_revision": 0, "max_allocations": 1}),
            serde_json::json!({"expected_revision": 0, "max_memory_bytes": 0}),
            serde_json::json!({"expected_revision": 0, "max_memory_bytes": brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES - 1}),
            serde_json::json!({"expected_revision": 0, "max_recipes_per_task": 0}),
            serde_json::json!({"expected_revision": 0, "max_recipes_per_task": i32::MAX as u64 + 1}),
            serde_json::json!({"expected_revision": 0, "failure_rollback_threshold": 65536}),
            serde_json::json!({"expected_revision": 0, "q4_retention_days": 0}),
            serde_json::json!({"expected_revision": u64::MAX}),
        ] {
            let patch = serde_json::from_value(invalid).unwrap();
            assert!(matches!(
                store.upsert("invalid-operator", "project", &patch).await,
                Err(MontyVmSettingsError::Invalid(_))
            ));
        }
        let client = rig.pool.get().await.unwrap();
        assert_eq!(client.query_one(
            "SELECT count(*) FROM reborn_monty_vm_settings WHERE tenant_id=$1 AND user_id=$2",
            &[&"settings-cas", &"invalid-operator"],
        ).await.unwrap().get::<_, i64>(0), 0);
        // A legacy/unversioned writer must not mutate settings while leaving
        // the revision unchanged: that would defeat the API's CAS protection.
        for query in [
            "UPDATE reborn_monty_vm_settings SET max_duration_secs=1200 WHERE tenant_id=$1 AND user_id=$2",
            "UPDATE reborn_monty_vm_settings SET max_recipes_per_task=17 WHERE tenant_id=$1 AND user_id=$2",
            "UPDATE reborn_monty_vm_settings SET revision=revision-1 WHERE tenant_id=$1 AND user_id=$2",
            "UPDATE reborn_monty_vm_settings SET revision=revision+2 WHERE tenant_id=$1 AND user_id=$2",
        ] {
            let error = client
                .execute(query, &[&"settings-cas", &"operator"])
                .await
                .unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
        }
        let preserved = store.get("operator", "project").await.unwrap();
        assert_eq!(preserved.revision, 2);
        assert_eq!(preserved.max_duration_secs, 900);
        assert_eq!(preserved.max_recipes_per_task, 16);
        // The graduation cursor is independent of operator configuration,
        // even when configuration revisions can no longer be incremented.
        client
            .execute(
                "INSERT INTO reborn_monty_vm_settings
             (tenant_id,user_id,agent_id,project_id,revision)
             VALUES ('settings-cas','exhausted-operator','agent','project',9223372036854775807)",
                &[],
            )
            .await
            .unwrap();
        client
            .execute(
                "UPDATE reborn_monty_vm_settings SET last_graduation_at=now()
             WHERE tenant_id='settings-cas' AND user_id='exhausted-operator'",
                &[],
            )
            .await
            .unwrap();
        let exhausted = client
            .query_one(
                "SELECT revision,last_graduation_at IS NOT NULL FROM reborn_monty_vm_settings
             WHERE tenant_id='settings-cas' AND user_id='exhausted-operator'",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(exhausted.get::<_, i64>(0), i64::MAX);
        assert!(exhausted.get::<_, bool>(1));
        let invalid = client.execute(
            "UPDATE reborn_monty_vm_settings SET last_graduation_at=now(),max_duration_secs=1200
             WHERE tenant_id='settings-cas' AND user_id='exhausted-operator'",
            &[],
        ).await.unwrap_err();
        assert_eq!(
            invalid.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        // A stale first-write claim must not leave a default row behind.
        let patch = serde_json::from_value(serde_json::json!({"expected_revision": 1})).unwrap();
        assert!(matches!(
            store.upsert("missing-operator", "project", &patch).await,
            Err(MontyVmSettingsError::RevisionConflict)
        ));
        assert_eq!(client.query_one(
            "SELECT count(*) FROM reborn_monty_vm_settings WHERE tenant_id=$1 AND user_id=$2",
            &[&"settings-cas", &"missing-operator"],
        ).await.unwrap().get::<_, i64>(0), 0);
        drop(client);
        drop(store);
        drop(rig);
    }
}
