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
