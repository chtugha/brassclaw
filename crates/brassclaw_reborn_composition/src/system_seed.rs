//! Conservative repair for legacy installation-seeded prompt rows.
//!
//! This helper repairs only required compiled-in legacy prompt projections.
//! It never classifies arbitrary source-labeled metadata as a boot prerequisite.
//! Older prompt rows are repaired only when the live prose hashes to the
//! digest compiled into this binary. It never rewrites prose and never
//! promotes a component whose content cannot be proven against that digest.

use brassclaw_pg::PgPool;

use crate::builtin_bootstrap::EXPECTED_CHECKSUMS;
use crate::checksum::sha256_hex;

/// Repair legacy prompt status only when the live prose hash matches the
/// compiled-in expected digest. Refresh the stored digest from that verified
/// content; this is a legacy projection, not immutable v3 approval.
///
/// Other component classes remain untouched. Their source/status labels are
/// neither bootstrap provenance nor proof that they belong to an active catalogue.
/// Active workflow qualification belongs to the subsequent catalogue owner.
pub(crate) async fn repair_packaged_legacy_prompts(
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
    // This repair owns only the exact packaged prompts selected above. Draft
    // metadata in other classes is not an active workflow and must not acquire
    // approval or block boot because of a source label. The subsequent global
    // catalogue owner verifies required active selections, exact approval and
    // registered implementations before workers/ingress start.
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}
