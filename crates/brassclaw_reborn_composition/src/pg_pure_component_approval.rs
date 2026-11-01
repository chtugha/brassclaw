//! Storage and verification of `pure-component-approval/1` records.
//!
//! A `PureComponentApprovalDeclaration` is an untrusted declaration that
//! references existing `reborn_component_review_evidence` records. This module
//! verifies that every referenced evidence record exists and has `succeeded=true`,
//! that every component in the closure matches `reborn_component_revisions`, and
//! then stores the approval in `reborn_pure_component_approvals` and its closure
//! in `reborn_pure_component_approval_closure`.
//!
//! The returned root UUID may be inserted into the `independently_qualified` set
//! in `authored_monty_catalogue`. Storing an approval does not activate any
//! catalogue entry or grant Tool permission; those remain separate gates.
//!
//! No public success-write API: only `record_pure_component_approval` may insert.

use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    pure_component_contract::{PureComponentApprovalDeclaration, PureComponentContractError},
    value_contract::ContractLimits,
};
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PureApprovalError {
    #[error("pure component approval: {0}")]
    Invalid(&'static str),
    #[error("pure component approval database operation failed")]
    Database(#[source] tokio_postgres::Error),
    #[error("pure component approval database connection failed")]
    Connection(#[source] deadpool_postgres::PoolError),
}
fn invalid(reason: &'static str) -> PureApprovalError {
    PureApprovalError::Invalid(reason)
}
fn db(error: tokio_postgres::Error) -> PureApprovalError {
    PureApprovalError::Database(error)
}

/// Read, verify and store one pure-component approval declaration.
///
/// Returns the approved root UUID on success. The caller must include this in
/// the `independently_qualified` set during catalogue preparation; it must never
/// be derived from request data or a caller-authored success flag.
///
/// # Invariants
///
/// - All evidence UUIDs must exist in `reborn_component_review_evidence` with
///   `succeeded=true` in the stored JSON.
/// - All component UUIDs/versions/checksums must match `reborn_component_revisions`.
/// - The component class_code for every closure member must be 22 (PythonCode).
/// - The root must be present in the closure.
/// - An existing stored approval with the same `approval_id` must be byte-identical.
/// - No evidence record is verified as Q1 or behavior by its format alone; the
///   stored `succeeded` flag in the evidence bytes is the only machine-readable gate.
///   Semantic agreement and human Q2 authenticity remain separate.
pub async fn record_pure_component_approval(
    pool: &PgPool,
    declaration: &PureComponentApprovalDeclaration,
) -> Result<Uuid, PureApprovalError> {
    let bytes = declaration.exact_bytes();
    let checksum = format!("{:x}", Sha256::digest(bytes.as_bytes()));
    let root = declaration.root();
    let components = declaration.components();
    let mode = declaration.mode();
    let q1 = declaration.q1_reference();
    let q2 = declaration.q2_reference();
    let behaviors = declaration.behavioral_references();

    // Collect all evidence UUIDs that must be verified.
    let mut evidence_ids: Vec<Uuid> = Vec::with_capacity(1 + behaviors.len());
    evidence_ids.push(q1);
    if let Some(q2_id) = q2 {
        evidence_ids.push(q2_id);
    }
    evidence_ids.extend_from_slice(behaviors);

    let mut client = pool.get().await.map_err(PureApprovalError::Connection)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(db)?;

    // 1. Verify all evidence records exist with succeeded=true.
    //    V102: reborn_component_review_evidence has evidence_bytes TEXT containing JSON.
    for &evidence_id in &evidence_ids {
        let row = tx
            .query_opt(
                "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
                &[&evidence_id],
            )
            .await
            .map_err(db)?
            .ok_or_else(|| invalid("referenced evidence record is missing"))?;
        let evidence_bytes: &str = row.get(0);
        let evidence_json: serde_json::Value = serde_json::from_str(evidence_bytes)
            .map_err(|_| invalid("evidence bytes are not valid JSON"))?;
        if evidence_json.get("succeeded").and_then(|v| v.as_bool()) != Some(true) {
            return Err(invalid("referenced evidence did not succeed"));
        }
    }

    // 2. Verify all components in the closure exist in reborn_component_revisions
    //    with matching version and checksum, and are class 22.
    let mut comp_ids: Vec<Uuid> = Vec::with_capacity(components.len());
    let mut comp_versions: Vec<i64> = Vec::with_capacity(components.len());
    let mut comp_classes: Vec<i16> = Vec::with_capacity(components.len());
    let mut comp_checksums: Vec<&str> = Vec::with_capacity(components.len());
    // We need a stable String backing for the checksums.
    let checksum_strings: Vec<String> = components
        .values()
        .map(|r| {
            r.checksum
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        })
        .collect();
    for (reference, hex_checksum) in components.values().zip(checksum_strings.iter()) {
        comp_ids.push(reference.uuid);
        comp_versions.push(
            i64::try_from(reference.version)
                .map_err(|_| invalid("component version exceeds i64 range"))?,
        );
        comp_classes.push(22i16); // always class 22 (enforced by PureComponentApprovalDeclaration)
        comp_checksums.push(hex_checksum.as_str());
    }
    // Use text[] for checksums since tokio_postgres doesn't have a native array type for &str easily
    let matching: bool = tx
        .query_one(
            "SELECT count(*) = cardinality($1::uuid[]) FROM
             unnest($1::uuid[], $2::bigint[], $3::smallint[], $4::text[])
             AS e(id, version, class_code, checksum)
             JOIN reborn_component_revisions r
               ON r.component_id = e.id AND r.version = e.version
              AND r.class_code = e.class_code AND r.checksum = e.checksum",
            &[
                &comp_ids,
                &comp_versions,
                &comp_classes,
                &comp_checksums
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
            ],
        )
        .await
        .map_err(db)?
        .get(0);
    if !matching || comp_ids.is_empty() {
        return Err(invalid("component closure does not match stored revisions"));
    }

    // 3. Verify the lengths satisfy the contract limits.
    if bytes.len() > REVISION_LIMITS.max_bytes {
        return Err(invalid("approval bytes exceed technical capacity"));
    }

    let root_hex: String = root.checksum.iter().map(|b| format!("{b:02x}")).collect();
    let mode_str = match mode {
        brassclaw_skills::association_contract::ValidationMode::Authored => "authored",
        brassclaw_skills::association_contract::ValidationMode::SystemSeed => "system_seed",
    };

    // 4. Insert the approval record (idempotent on conflict).
    tx.execute(
        "INSERT INTO reborn_pure_component_approvals
         (approval_id, root_id, root_version, root_checksum, validation_mode,
          q1_ref, q2_ref, approval_bytes, checksum)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (approval_id) DO NOTHING",
        &[
            &declaration.approval_id(),
            &root.uuid,
            &i64::try_from(root.version).map_err(|_| invalid("root version exceeds i64 range"))?,
            &root_hex,
            &mode_str,
            &q1,
            &q2,
            &bytes,
            &checksum,
        ],
    )
    .await
    .map_err(db)?;

    // Verify idempotency: stored bytes must be byte-identical to what we computed.
    let stored = tx
        .query_one(
            "SELECT approval_bytes, checksum FROM reborn_pure_component_approvals WHERE approval_id=$1",
            &[&declaration.approval_id()],
        )
        .await
        .map_err(db)?;
    if stored.get::<_, &str>(0) != bytes || stored.get::<_, &str>(1) != checksum {
        return Err(invalid(
            "stored pure-component approval has conflicting bytes",
        ));
    }

    // 5. Insert closure rows (idempotent on conflict).
    for (reference, hex_checksum) in components.values().zip(checksum_strings.iter()) {
        let version = i64::try_from(reference.version)
            .map_err(|_| invalid("component version exceeds i64 range"))?;
        tx.execute(
            "INSERT INTO reborn_pure_component_approval_closure
             (approval_id, component_id, component_version, component_checksum)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (approval_id, component_id) DO NOTHING",
            &[
                &declaration.approval_id(),
                &reference.uuid,
                &version,
                &hex_checksum,
            ],
        )
        .await
        .map_err(db)?;
    }

    tx.commit().await.map_err(db)?;
    Ok(root.uuid)
}

/// Read an existing approved pure-component root UUID by approval ID.
/// Returns `None` if not found. Does not re-verify evidence; that happens at
/// `record_pure_component_approval` time. The caller is responsible for
/// matching the exact approval ID against the expected declaration.
pub async fn read_pure_component_approval_root(
    pool: &PgPool,
    approval_id: Uuid,
) -> Result<Option<Uuid>, PureApprovalError> {
    let client = pool.get().await.map_err(PureApprovalError::Connection)?;
    let row = client
        .query_opt(
            "SELECT root_id FROM reborn_pure_component_approvals WHERE approval_id=$1",
            &[&approval_id],
        )
        .await
        .map_err(db)?;
    Ok(row.map(|r| r.get(0)))
}

/// Parse a `pure-component-approval/1` JSON declaration from bytes and
/// immediately record it. Returns the approved root UUID.
pub async fn parse_and_record_pure_component_approval(
    pool: &PgPool,
    json_bytes: &str,
) -> Result<Uuid, PureApprovalError> {
    let limits = ContractLimits {
        max_depth: REVISION_LIMITS.max_depth,
        max_nodes: REVISION_LIMITS.max_nodes,
        max_bytes: REVISION_LIMITS.max_bytes,
    };
    let declaration = PureComponentApprovalDeclaration::from_json(json_bytes, limits).map_err(
        |error| match error {
            PureComponentContractError::Invalid(reason) => invalid(reason),
            PureComponentContractError::Data(_) => {
                invalid("approval bytes exceed technical data contract limits")
            }
        },
    )?;
    record_pure_component_approval(pool, &declaration).await
}

/// Read the set of approved pure-component root UUIDs for the given component
/// IDs within a transaction. Used by `authored_monty_catalogue` to populate
/// the `independently_qualified` set from stored approval records.
///
/// Returns only roots that have a verified approval record. Components with no
/// stored approval are absent from the returned set; missing approval is not an
/// error here — it means the component is not independently qualified via this
/// path.
pub async fn load_independently_qualified(
    tx: &tokio_postgres::Transaction<'_>,
    component_ids: &[Uuid],
) -> Result<std::collections::BTreeSet<Uuid>, PureApprovalError> {
    if component_ids.is_empty() {
        return Ok(std::collections::BTreeSet::new());
    }
    // Find all approvals whose root_id is in the given component set.
    // We match by root_id because the component is the root of its own closure.
    let rows = tx
        .query(
            "SELECT DISTINCT root_id FROM reborn_pure_component_approvals
             WHERE root_id = ANY($1::uuid[])",
            &[&component_ids],
        )
        .await
        .map_err(db)?;
    Ok(rows.iter().map(|row| row.get::<_, Uuid>(0)).collect())
}

/// Record an immutable link between a review submission and a pure-component
/// approval, using the new `reborn_pure_evidence_submission_links` table (V201).
///
/// This is the write path for pure-component-review cutover contract item 1:
/// it ties the original `submission_id` (from `reborn_component_review_submissions`)
/// to the `approval_id` (from `reborn_pure_component_approvals`) and the exact
/// `component_id`/`candidate_version` pair from the submission. Doing so lets
/// the trusted review owner verify that the pure approval was derived from the
/// correct exact review subject without reparsing `subject_bytes`.
///
/// Idempotent: a repeated call with the same `link_id` succeeds if the stored
/// row is byte-identical. Conflicting bytes (same `link_id`, different content)
/// are rejected.
///
/// # Invariants
///
/// - All three IDs are non-nil UUIDs.
/// - `submission_id` must exist in `reborn_component_review_submissions`.
/// - `approval_id` must exist in `reborn_pure_component_approvals`.
/// - `component_id`/`candidate_version` must exist in `reborn_component_revisions`
///   and must match the submission's own `component_id`/`candidate_version`.
/// - This function does not verify evidence or grant any approval; those are
///   separate gates in the trusted review owner.
pub async fn record_pure_evidence_submission_link(
    pool: &PgPool,
    link_id: Uuid,
    submission_id: Uuid,
    approval_id: Uuid,
    component_id: Uuid,
    candidate_version: u64,
) -> Result<(), PureApprovalError> {
    if link_id.is_nil() || submission_id.is_nil() || approval_id.is_nil() || component_id.is_nil()
    {
        return Err(invalid("link, submission, approval and component IDs must be non-nil"));
    }
    let version =
        i64::try_from(candidate_version).map_err(|_| invalid("candidate version exceeds i64 range"))?;
    let mut client = pool.get().await.map_err(PureApprovalError::Connection)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(db)?;

    // Verify the submission exists and the component_id/candidate_version match.
    let sub_row = tx
        .query_opt(
            "SELECT component_id, candidate_version
             FROM reborn_component_review_submissions
             WHERE submission_id = $1",
            &[&submission_id],
        )
        .await
        .map_err(db)?
        .ok_or_else(|| invalid("submission_id not found in review submissions"))?;
    let stored_component: Uuid = sub_row.get(0);
    let stored_version: i64 = sub_row.get(1);
    if stored_component != component_id || stored_version != version {
        return Err(invalid(
            "component_id/candidate_version do not match the submission's subject",
        ));
    }

    // Verify the approval exists.
    let approval_exists: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM reborn_pure_component_approvals WHERE approval_id = $1)",
            &[&approval_id],
        )
        .await
        .map_err(db)?
        .get(0);
    if !approval_exists {
        return Err(invalid("approval_id not found in pure component approvals"));
    }

    // Insert idempotently.
    tx.execute(
        "INSERT INTO reborn_pure_evidence_submission_links
         (link_id, submission_id, approval_id, component_id, candidate_version)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (link_id) DO NOTHING",
        &[&link_id, &submission_id, &approval_id, &component_id, &version],
    )
    .await
    .map_err(db)?;

    // Verify idempotency: if a row already exists it must have identical fields.
    let stored = tx
        .query_one(
            "SELECT submission_id, approval_id, component_id, candidate_version
             FROM reborn_pure_evidence_submission_links WHERE link_id = $1",
            &[&link_id],
        )
        .await
        .map_err(db)?;
    if stored.get::<_, Uuid>(0) != submission_id
        || stored.get::<_, Uuid>(1) != approval_id
        || stored.get::<_, Uuid>(2) != component_id
        || stored.get::<_, i64>(3) != version
    {
        return Err(invalid(
            "stored pure evidence submission link has conflicting fields",
        ));
    }

    tx.commit().await.map_err(db)
}

#[cfg(all(test, feature = "skills-db"))]
mod tests {
    use super::*;
    use brassclaw_skills::{
        pure_component_contract::PureComponentApprovalDeclaration, value_contract::ContractLimits,
    };
    use serde_json::json;

    fn limits() -> ContractLimits {
        ContractLimits {
            max_depth: REVISION_LIMITS.max_depth,
            max_nodes: REVISION_LIMITS.max_nodes,
            max_bytes: REVISION_LIMITS.max_bytes,
        }
    }

    fn fake_checksum() -> String {
        "a".repeat(64)
    }

    fn declaration_json(
        approval_id: Uuid,
        root_id: Uuid,
        version: u64,
        q1: Uuid,
        behavioral: Uuid,
    ) -> String {
        json!({
            "format": "pure-component-approval/1",
            "approval_id": approval_id.to_string(),
            "root": {
                "uuid": root_id.to_string(),
                "class_code": 22,
                "version": version,
                "checksum": fake_checksum(),
            },
            "components": [{
                "uuid": root_id.to_string(),
                "class_code": 22,
                "version": version,
                "checksum": fake_checksum(),
            }],
            "validation_mode": "authored",
            "q1_ref": q1.to_string(),
            "q2_ref": Uuid::new_v4().to_string(),
            "behavioral_refs": [behavioral.to_string()],
        })
        .to_string()
    }

    #[tokio::test]
    async fn missing_evidence_record_is_rejected() {
        let rig = crate::runtime::test_pg::pg_rig().await;
        let q1 = Uuid::new_v4();
        let behavioral = Uuid::new_v4();
        let root_id = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let json = declaration_json(approval_id, root_id, 1, q1, behavioral);
        let decl = PureComponentApprovalDeclaration::from_json(&json, limits()).unwrap();
        // No evidence records exist — must fail.
        let result = record_pure_component_approval(&rig.pool, &decl).await;
        assert!(
            matches!(result, Err(PureApprovalError::Invalid(msg)) if msg.contains("evidence record is missing")),
            "expected missing-evidence error, got: {result:?}"
        );
    }
}
