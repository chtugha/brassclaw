//! Shared component boot prerequisites for every long-running product entry.
//! Migrations precede this function. No worker/producer may start until it
//! succeeds; WebUI construction must never supply missing runtime prerequisites.

use crate::{booted_db::BootedDb, error::RebornBuildError};

pub(crate) async fn initialize_runtime_components(
    booted_db: &BootedDb,
    host_tenant_id: &str,
) -> Result<(), RebornBuildError> {
    crate::seed_builtin_host::seed_builtin_host_components(booted_db, host_tenant_id)
        .await
        .map_err(|e| crate::error::RebornBuildError::InvalidConfig {
            reason: format!("builtin host component seeding failed: {e}"),
        })?;
    crate::builtin_bootstrap::seed_builtin_components(booted_db, host_tenant_id)
        .await
        .map_err(|e| crate::error::RebornBuildError::InvalidConfig {
            reason: format!("builtin component seeding failed: {e}"),
        })?;
    let recovered = crate::boot_integrity::run_boot_integrity_check(booted_db)
        .await
        .map_err(|e| crate::error::RebornBuildError::InvalidConfig {
            reason: format!("component validation queue recovery failed: {e}"),
        })?;
    if recovered > 0 {
        tracing::warn!(
            recovered,
            "boot integrity: recovered components missing from validation queue"
        );
    }

    // Step 4 — content integrity check.
    // Verifies stored checksums for source='system' prose rows and also
    // checks compiled-in prompt seeds against their expected checksums.
    // Hard error on mismatch — no runtime worker is started.
    // `brassclaw repair` reviews compiled-in prompt seeds and confirms before replacing differing content.
    {
        match crate::content_integrity::run_content_integrity_check(booted_db).await {
            Ok(crate::content_integrity::ContentIntegrityOutcome::Ok { checked }) => {
                tracing::debug!(checked, "content integrity: all system components verified");
            }
            Ok(crate::content_integrity::ContentIntegrityOutcome::Corrupted(mismatches)) => {
                for m in &mismatches {
                    tracing::error!(
                        table = m.table,
                        name = %m.name,
                        expected = %m.expected,
                        actual = %m.actual,
                        "CONTENT INTEGRITY FAILURE: system component checksum mismatch; review the component before repair."
                    );
                }
                return Err(crate::error::RebornBuildError::ContentIntegrity(
                    crate::content_integrity::ContentIntegrityError::Corrupted(mismatches),
                ));
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "content integrity check failed; aborting boot"
                );
                return Err(e.into());
            }
        }
    }

    // Retain the exact new global-root draft without graduating the legacy
    // class-10 row or changing any active/operator selection. Approval and the
    // coordinated global lifecycle cutover remain independent prerequisites.
    #[cfg(feature = "skills-db")]
    crate::global_root_seed::retain_packaged_global_root(
        booted_db.pool().clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("global orchestrator draft retention failed: {error}"),
    })?;

    // Load every required prompt before mutating process-wide OnceLocks. A
    // failed database read must not leave a partially initialized prompt set.
    let failure = load_required_prompt(booted_db, host_tenant_id, "failure_explanation").await?;
    let compaction =
        load_required_prompt(booted_db, host_tenant_id, "compaction_summarizer_fresh").await?;
    #[cfg(feature = "root-llm-provider")]
    let sempai = load_required_prompt(booted_db, host_tenant_id, "sempai_audit").await?;
    let general =
        load_required_prompt(booted_db, host_tenant_id, "subagent:direction:general").await?;
    let researcher =
        load_required_prompt(booted_db, host_tenant_id, "subagent:direction:researcher").await?;
    let explorer =
        load_required_prompt(booted_db, host_tenant_id, "subagent:direction:explorer").await?;
    let coder = load_required_prompt(booted_db, host_tenant_id, "subagent:direction:coder").await?;

    brassclaw_loop_support::init_failure_explanation_prompt(failure);
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(compaction);
    #[cfg(feature = "root-llm-provider")]
    brassclaw_reborn::loop_driver_host::init_sempai_persona(sempai);
    brassclaw_reborn::subagent::directions::init_directions(general, researcher, explorer, coder);

    Ok(())
}

/// Load the body of a validated `source='system'` skill row from the DB.
///
/// Used in the boot chain to populate `OnceLock` prompt accessors after
/// `run_content_integrity_check` has verified the rows are uncorrupted.
///
/// Returns `Err` if the pool checkout fails, the row is missing, or the query
/// fails. A missing row indicates seeding did not run or was rolled back.
#[cfg(feature = "postgres")]
async fn load_system_skill_body(
    booted_db: &crate::booted_db::BootedDb,
    tenant_id: &str,
    name: &str,
) -> Result<String, String> {
    let client = booted_db
        .pool()
        .get()
        .await
        .map_err(|e| format!("pool checkout failed loading system skill '{name}': {e}"))?;
    let row = client
        .query_one(
            "SELECT body, content_checksum FROM reborn_skills
              WHERE tenant_id = $1 AND name = $2
                AND source = 'system' AND validation_status = 'validated'
              LIMIT 1",
            &[&tenant_id, &name],
        )
        .await
        .map_err(|e| format!("DB query failed loading system skill '{name}': {e}"))?;
    let body = row.get::<_, String>(0);
    let checksum = row
        .get::<_, Option<String>>(1)
        .ok_or_else(|| format!("required system skill '{name}' has no content checksum"))?;
    // Verify the actual body being installed, not only an earlier DB snapshot.
    // An edit between the global integrity scan and this read must fail boot.
    let mut mismatches = Vec::new();
    crate::content_integrity::check_row("reborn_skills", name, &body, &checksum, &mut mismatches);
    if !mismatches.is_empty() {
        return Err(format!(
            "required system skill '{name}' failed content verification"
        ));
    }
    Ok(body)
}

async fn load_required_prompt(
    booted_db: &BootedDb,
    tenant_id: &str,
    name: &str,
) -> Result<String, RebornBuildError> {
    load_system_skill_body(booted_db, tenant_id, name)
        .await
        .map_err(|e| RebornBuildError::InvalidConfig {
            reason: format!("failed to load required system skill {name}: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn prefix_capability_migration_preserves_identity_and_overrides() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.expect("test database connection");
        // Include both repair targets and rows that must remain untouched.
        client.batch_execute(
            "INSERT INTO reborn_tools
             (tenant_id, user_id, agent_id, project_id, name, description, source, capability_id)
             VALUES
             ('migration', 'system', 'system', 'system', 'host.sweep_validated_components', 'legacy', 'system', 'host.sweep_validated_components'),
             ('migration', 'system', 'system', 'system', 'host.store_prefix_bundle', 'legacy', 'system', 'host.store_prefix_bundle'),
             ('migration', 'authored', 'system', 'system', 'host.sweep_validated_components', 'authored', 'authored', 'host.sweep_validated_components'),
             ('migration', 'override', 'system', 'system', 'host.store_prefix_bundle', 'override', 'system', 'custom.store')"
        ).await.expect("legacy fixture");
        let before = client.query(
            "SELECT id, name, user_id FROM reborn_tools WHERE tenant_id = 'migration' ORDER BY user_id, name", &[]
        ).await.expect("original identities");
        let migration = include_str!(
            "../../brassclaw_pg/migrations/V088__prefix_bundle_capability_namespace.sql"
        );
        client
            .batch_execute(migration)
            .await
            .expect("repair mappings");
        client
            .batch_execute(migration)
            .await
            .expect("idempotent repair");
        let after = client.query(
            "SELECT id, name, user_id, capability_id FROM reborn_tools WHERE tenant_id = 'migration' ORDER BY user_id, name", &[]
        ).await.expect("repaired mappings");
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(&after) {
            assert_eq!(
                before.get::<_, uuid::Uuid>(0),
                after.get::<_, uuid::Uuid>(0)
            );
            let name: String = after.get(1);
            assert_eq!(before.get::<_, String>(1), name);
            let user: String = after.get(2);
            let capability: String = after.get(3);
            match user.as_str() {
                "system" => assert_eq!(capability, name.replacen("host.", "builtin.", 1)),
                "authored" => assert_eq!(capability, name),
                "override" => assert_eq!(capability, "custom.store"),
                other => panic!("unexpected fixture identity: {other}"),
            }
        }
    }

    /// Exercise boot without a WebUI, worker, conversation or fabricated turn.
    /// Database failures must fail this test rather than silently skip it.
    #[tokio::test]
    async fn native_database_boot_is_idempotent_and_rejects_changed_prompt() {
        // Reuse the native database fixture: composition tests do not own
        // listeners or duplicate PostgreSQL startup/cleanup conventions.
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = Arc::clone(&rig.pool);
        let booted = crate::booted_db::run_migrations_and_return_booted_db(pool.clone())
            .await
            .expect("schema migrations");
        let tenant = "component-boot-test";
        initialize_runtime_components(&booted, tenant)
            .await
            .expect("runtime components boot without ingress");
        initialize_runtime_components(&booted, tenant)
            .await
            .expect("repeated boot preserves validated components");
        #[cfg(feature = "skills-db")]
        {
            let retained_version: i64 = pool
                .get()
                .await
                .expect("boot verification connection")
                .query_one(
                    "SELECT version FROM reborn_component_revisions
                     WHERE class_code=10 AND
                     revision_bytes::jsonb #>> '{document,name}'='orchestrator:global'",
                    &[],
                )
                .await
                .expect("shared startup retained one root before any review/VM caller")
                .get(0);
            assert_eq!(retained_version, 1);
            let root = crate::global_root_seed::retain_packaged_global_root(
                pool.clone(),
                crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
            )
            .await
            .expect("boot retained the exact packaged root draft");
            assert_eq!(root.reference().version, 1);
            assert_eq!(
                root.source(),
                include_str!("../../brassclaw_engine/orchestrator/global_mode.py")
            );
            assert!(!root.ports().contains("post_reply"));
        }
        let expected = load_required_prompt(&booted, tenant, "failure_explanation")
            .await
            .expect("verified prompt");
        assert!(!expected.is_empty());
        let client = pool.get().await.expect("test database connection");
        let updated = client
            .execute(
                "UPDATE reborn_skills SET body = body || 'tampered' \
                 WHERE tenant_id = $1 AND name = 'failure_explanation'",
                &[&tenant],
            )
            .await
            .expect("simulate edit after integrity scan");
        assert_eq!(updated, 1);
        assert!(
            load_required_prompt(&booted, tenant, "failure_explanation")
                .await
                .is_err()
        );
        assert!(
            initialize_runtime_components(&booted, tenant)
                .await
                .is_err()
        );
        drop(client);
        drop(booted);
        pool.close();
        drop(pool);
        drop(rig); // supervised shutdown precedes temporary-directory removal
    }
}
