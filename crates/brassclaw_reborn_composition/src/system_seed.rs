//! Conservative repair for legacy installation-seeded prompt rows.
//!
//! Seeders now write trusted built-ins as validated at insertion time. This
//! helper only repairs older prompt rows when the live prose hashes to the
//! digest compiled into this binary. It never rewrites prose and never
//! promotes a component whose content cannot be proven against that digest.

use brassclaw_pg::PgPool;

use crate::builtin_bootstrap::EXPECTED_CHECKSUMS;
use crate::checksum::sha256_hex;

/// Promote legacy seeded prompt rows only when all three checksum values
/// (database prose and compiled-in expected digest) match. The stored digest
/// is then refreshed from that verified content.
///
/// Other component classes intentionally remain untouched: they do not yet
/// have a canonical seed checksum, so their provenance cannot be proven here.
pub(crate) async fn ensure_system_seeds_validated(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
    agent_id: &str,
    project_id: &str,
) -> Result<(), String> {
    let names: Vec<String> = EXPECTED_CHECKSUMS
        .keys()
        .map(|name| (*name).to_string())
        .collect();
    let mut client = pool.get().await.map_err(|error| error.to_string())?;
    let transaction = client
        .transaction()
        .await
        .map_err(|error| error.to_string())?;
    let rows = transaction
        .query(
            "SELECT id, name, body, validation_status
               FROM reborn_skills
              WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3
                AND project_id = $4 AND source = 'system' AND name = ANY($5)
              FOR UPDATE",
            &[&tenant_id, &user_id, &agent_id, &project_id, &names],
        )
        .await
        .map_err(|error| error.to_string())?;

    let mut verified_pending = Vec::new();
    let mut mismatched_pending = Vec::new();
    for row in &rows {
        let id: uuid::Uuid = row.get(0);
        let name: String = row.get(1);
        let body: String = row.get(2);
        let validation_status: String = row.get(3);
        if validation_status == "validated" {
            continue;
        }
        if validation_status != "pending" {
            mismatched_pending.push(format!("{name} (status={validation_status})"));
            continue;
        }

        let expected = EXPECTED_CHECKSUMS
            .get(name.as_str())
            .expect("query is restricted to expected checksum names");
        let actual = sha256_hex(&body);
        if actual == *expected {
            verified_pending.push((id, expected.clone()));
        } else {
            mismatched_pending.push(format!("{name} (checksum mismatch)"));
        }
    }

    if !mismatched_pending.is_empty() {
        return Err(format!(
            "refusing automatic promotion because component status/content is not an exact pending installation seed: {}; review or explicitly repair these components",
            mismatched_pending.join(", ")
        ));
    }

    for (id, expected) in verified_pending {
        transaction
            .execute(
                "UPDATE reborn_skills SET validation_status = 'validated', content_checksum = $2, updated_at = now()
                  WHERE id = $1 AND validation_status <> 'validated'",
                &[&id, &expected],
            )
            .await
            .map_err(|error| error.to_string())?;
    }
    // Do not silently leave a legacy installation row unavailable. These
    // classes do not have compiled-in row checksums, so report them without
    // changing their validation state or content.
    let pending = transaction
        .query(
            "SELECT table_name, name, validation_status FROM (
                 SELECT 'reborn_skills' AS table_name, name, validation_status FROM reborn_skills WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_tools', name, validation_status FROM reborn_tools WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_actions', name, validation_status FROM reborn_actions WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_specs', name, validation_status FROM reborn_specs WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_summaries', name, validation_status FROM reborn_summaries WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_lessons', name, validation_status FROM reborn_lessons WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_issues', name, validation_status FROM reborn_issues WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_notes', name, validation_status FROM reborn_notes WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_tool_skills', name, validation_status FROM reborn_tool_skills WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_plans', name, validation_status FROM reborn_plans WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_docus', name, validation_status FROM reborn_docus WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_python_code', name, validation_status FROM reborn_python_code WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_recipes', name, validation_status FROM reborn_recipes WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_extensions_unified', name, validation_status FROM reborn_extensions_unified WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 UNION ALL SELECT 'reborn_extension_catalogues', name, validation_status FROM reborn_extension_catalogues WHERE source='system' AND validation_status <> 'validated' AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
             ) pending ORDER BY table_name, name",
            &[&tenant_id, &user_id, &agent_id, &project_id],
        )
        .await
        .map_err(|error| error.to_string())?;
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;
    if !pending.is_empty() {
        let names = pending
            .iter()
            .map(|row| {
                format!(
                    "{}:{} (status={})",
                    row.get::<_, String>(0),
                    row.get::<_, String>(1),
                    row.get::<_, String>(2)
                )
            })
            .collect::<Vec<_>>();
        return Err(format!(
            "installation-seeded components remain unvalidated and were left unchanged because their content/status could not be safely repaired: {}",
            names.join(", ")
        ));
    }
    Ok(())
}
