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
        // Schema constraints guarantee these signed values are nonnegative.
        Ok(MontyVmSettings {
            execution_limits,
            revision: row.get::<_, i64>(9) as u64,
            max_duration_secs: row.get::<_, i32>(0) as u64,
            max_allocations: None,
            allocation_count_limit_supported: false,
            retired_max_allocations: row.get::<_, Option<i64>>(1).map(|value| value as u64),
            max_memory_bytes: Some(row.get::<_, i64>(2) as u64),
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
                            forensic_packet_retention_days, token_budgets_enabled, revision, execution_limits
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
            let duration = checked_i32(update.max_duration_secs, "max_duration_secs")?;
            if duration.is_some_and(|value| !(30..=3600).contains(&value)) {
                return Err(MontyVmSettingsError::Invalid(
                    "max_duration_secs must be 30..=3600".into(),
                ));
            }
            if update.max_allocations.is_some() {
                return Err(MontyVmSettingsError::Invalid(
                    "max_allocations was removed by Monty 1.0; historical values are read-only"
                        .into(),
                ));
            }
            let memory = checked_positive_i64(update.max_memory_bytes, "max_memory_bytes")?;
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
                 revision = revision + 1
                 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                   AND revision=$13
                 RETURNING max_duration_secs, retired_max_allocations, max_memory_bytes,
                   failure_rollback_threshold, active_orchestrator_id,
                   prior_knowledge_token_budget, q4_retention_days,
                   forensic_packet_retention_days, token_budgets_enabled, revision, execution_limits",
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
    async fn native_monty_settings_patch_is_revision_checked_and_atomic() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let store = PgMontyVmSettingsStore::new(Arc::clone(&rig.pool), "settings-cas", "agent");
        let duration: UpdateMontyVmSettingsRequest = serde_json::from_value(serde_json::json!({
            "expected_revision": 0, "max_duration_secs": 900
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
        assert!(second.token_budgets_enabled);
        assert_eq!(second.prior_knowledge_token_budget, 100_000);
        assert!(matches!(
            store.upsert("operator", "project", &retry).await,
            Err(MontyVmSettingsError::RevisionConflict)
        ));
        let after = store.get("operator", "project").await.unwrap();
        assert_eq!(after.revision, second.revision);
        assert_eq!(after.max_duration_secs, second.max_duration_secs);
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
            serde_json::json!({"expected_revision": 0, "max_allocations": u64::MAX}),
            serde_json::json!({"expected_revision": 0, "max_allocations": 1}),
            serde_json::json!({"expected_revision": 0, "max_memory_bytes": 0}),
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
