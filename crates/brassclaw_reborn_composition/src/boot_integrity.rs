//! Boot-time integrity check — Phase N (N.5).
//!
//! Finds component rows that are NOT in `'validated'` state but also have
//! NO matching row in `reborn_validation_queue`, i.e. components that somehow
//! lost their queue entry (crash-recovery, manual import, restored backup, or
//! any save path that bypassed [`ValidationQueueStore::submit`]).
//!
//! Such rows are *inconsistent*: they are not validated but are also not tracked
//! by the queue, so Q1/Q2 will never run for them.  The check finds them, logs a
//! warning for each, and calls `submit()` to re-enqueue them at state 1 so the
//! normal Q1→Q2 graduation path can proceed.
//!
//! # Scope
//!
//! The check covers all 15 component tables across **all tenants/users/scopes**
//! present in the DB. It does not limit to the host tenant.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

#![forbid(unsafe_code)]

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::validation_queue::{ValidationQueueError, ValidationQueueStore};

/// Error raised by [`run_boot_integrity_check`].
#[derive(Debug, Error)]
pub(crate) enum BootIntegrityError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("validation queue submit failed: {0}")]
    Queue(#[from] ValidationQueueError),
}

/// One unqueued component found during the integrity sweep.
#[derive(Debug)]
struct UnqueuedComponent {
    id: Uuid,
    tenant_id: String,
    user_id: String,
    agent_id: String,
    project_id: String,
    class_code: i16,
    table_label: &'static str,
}

// ---------------------------------------------------------------------------
// SQL helpers
// ---------------------------------------------------------------------------

/// Build the UNION ALL fragment for a table that carries a fixed `class_code`.
///
/// The subquery anti-join matches the queue row on all four scope columns so
/// only the component's own scope is checked — not cross-scope.
fn fixed_class_arm(table: &'static str, class_code: i16) -> String {
    format!(
        "SELECT id, tenant_id, user_id, agent_id, project_id, \
                {class_code}::SMALLINT AS class_code, \
                '{table}'  AS table_label \
           FROM {table} \
          WHERE validation_status != 'validated' \
            AND id NOT IN (\
                SELECT component_id FROM reborn_validation_queue q \
                 WHERE q.tenant_id  = {table}.tenant_id \
                   AND q.user_id    = {table}.user_id \
                   AND q.agent_id   = {table}.agent_id \
                   AND q.project_id = {table}.project_id)"
    )
}

/// Build the UNION ALL fragment for a table that carries a variable `class_code`
/// column (e.g. `reborn_skills`, `reborn_extensions_unified`).
fn variable_class_arm(table: &'static str) -> String {
    format!(
        "SELECT id, tenant_id, user_id, agent_id, project_id, \
                class_code::SMALLINT, \
                '{table}' AS table_label \
           FROM {table} \
          WHERE validation_status != 'validated' \
            AND id NOT IN (\
                SELECT component_id FROM reborn_validation_queue q \
                 WHERE q.tenant_id  = {table}.tenant_id \
                   AND q.user_id    = {table}.user_id \
                   AND q.agent_id   = {table}.agent_id \
                   AND q.project_id = {table}.project_id)"
    )
}

/// Build the full UNION ALL query over all 15 component tables.
fn build_integrity_query() -> String {
    let arms: Vec<String> = vec![
        // Variable class_code tables
        variable_class_arm("reborn_skills"),           // classes 1/2/3/10/50
        variable_class_arm("reborn_extensions_unified"), // classes 4-9
        // Fixed class_code tables
        fixed_class_arm("reborn_tools", 0),
        fixed_class_arm("reborn_tool_skills", 13),
        fixed_class_arm("reborn_actions", 16),
        fixed_class_arm("reborn_recipes", 21),
        fixed_class_arm("reborn_specs", 12),
        fixed_class_arm("reborn_plans", 14),
        fixed_class_arm("reborn_summaries", 15),
        fixed_class_arm("reborn_docus", 17),
        fixed_class_arm("reborn_lessons", 18),
        fixed_class_arm("reborn_issues", 19),
        fixed_class_arm("reborn_notes", 20),
        fixed_class_arm("reborn_python_code", 22),        // V052 / class 22
        fixed_class_arm("reborn_extension_catalogues", 23), // V053 / class 23
    ];
    arms.join(" UNION ALL ")
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the boot-time integrity check across all 15 component tables.
///
/// Finds every component row that is not `'validated'` and has no matching
/// `reborn_validation_queue` entry for its scope.  For each such row:
/// - logs a `WARN` with component ID, class, and source table
/// - calls `ValidationQueueStore::submit` to re-enqueue at state 1
///
/// All errors are surfaced; partial recovery (some submits succeed, some fail)
/// is acceptable — the next boot will retry the remaining rows.
///
/// # Performance
///
/// One DB round-trip for the UNION ALL scan, then one `submit` per missing row.
/// In steady state (V077 populate ran successfully) there should be zero missing
/// rows and the function returns immediately after the scan.
pub(crate) async fn run_boot_integrity_check(pool: &std::sync::Arc<PgPool>) -> Result<u64, BootIntegrityError> {
    let client = pool.get().await.map_err(|e| BootIntegrityError::Pool {
        reason: e.to_string(),
    })?;

    let sql = build_integrity_query();
    let rows = client.query(&sql, &[]).await.map_err(|e| BootIntegrityError::Db {
        reason: e.to_string(),
    })?;

    if rows.is_empty() {
        tracing::debug!("boot integrity check: all component tables consistent");
        return Ok(0);
    }

    let mut components: Vec<UnqueuedComponent> = Vec::with_capacity(rows.len());
    for row in &rows {
        let table_label_str: String = row.get(6);
        // SAFETY: static strings were supplied in fixed_class_arm / variable_class_arm;
        // we match back to the static slice by string comparison.
        let table_label: &'static str = table_label_for(table_label_str.as_str());
        components.push(UnqueuedComponent {
            id: row.get(0),
            tenant_id: row.get(1),
            user_id: row.get(2),
            agent_id: row.get(3),
            project_id: row.get(4),
            class_code: row.get(5),
            table_label,
        });
    }

    tracing::warn!(
        count = components.len(),
        "boot integrity check: found non-validated components without queue entries; \
         re-submitting to Q1 queue"
    );

    // Re-enqueue each missing component at state 1.
    let store = ValidationQueueStore::new(std::sync::Arc::clone(pool));
    let mut recovered: u64 = 0;
    for comp in &components {
        let scope = ComponentScope {
            tenant_id: comp.tenant_id.clone(),
            user_id: comp.user_id.clone(),
            agent_id: comp.agent_id.clone(),
            project_id: comp.project_id.clone(),
        };
        tracing::warn!(
            component_id  = %comp.id,
            class_code    = comp.class_code,
            table         = comp.table_label,
            tenant_id     = %comp.tenant_id,
            "boot integrity: re-queuing missing component at Q1_pending"
        );
        match store
            .submit(
                &scope,
                comp.id,
                comp.class_code.into(),
                None, // no upgrade payload — this is a recovery submission
            )
            .await
        {
            Ok(()) => recovered += 1,
            Err(ValidationQueueError::AlreadyQueued { .. }) => {
                // Race: another process submitted it between our scan and now.
                recovered += 1;
            }
            Err(e) => {
                tracing::warn!(
                    component_id = %comp.id,
                    error = %e,
                    "boot integrity: failed to re-queue component (will retry on next boot)"
                );
            }
        }
    }

    Ok(recovered)
}

/// Map a runtime table-label string back to the nearest static `&str`.
///
/// Since the table labels were written into SQL as string literals there is no
/// `&'static str` coming back from Postgres.  This lookup maps the returned
/// `String` to a static slice for logging purposes only.  Any unknown value
/// falls back to `"unknown"` gracefully.
fn table_label_for(s: &str) -> &'static str {
    match s {
        "reborn_skills" => "reborn_skills",
        "reborn_extensions_unified" => "reborn_extensions_unified",
        "reborn_tools" => "reborn_tools",
        "reborn_tool_skills" => "reborn_tool_skills",
        "reborn_actions" => "reborn_actions",
        "reborn_recipes" => "reborn_recipes",
        "reborn_specs" => "reborn_specs",
        "reborn_plans" => "reborn_plans",
        "reborn_summaries" => "reborn_summaries",
        "reborn_docus" => "reborn_docus",
        "reborn_lessons" => "reborn_lessons",
        "reborn_issues" => "reborn_issues",
        "reborn_notes" => "reborn_notes",
        "reborn_python_code" => "reborn_python_code",
        "reborn_extension_catalogues" => "reborn_extension_catalogues",
        _ => "unknown",
    }
}
