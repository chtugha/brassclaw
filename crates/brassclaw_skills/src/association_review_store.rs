//! Exact immutable association/review record resolution. This read-only store
//! supplies no authoring/approval write API, activation or Tool authority.
//! Trusted Q1, behavioral and authenticated human-Q2 producers must still issue
//! these records; legacy labels/receipts cannot substitute for those producers.

use std::collections::{BTreeMap, BTreeSet};

use brassclaw_pg::PgError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::{
    association_contract::{
        AssociationApprovalDeclaration, ComponentRevisionRef, SkillAssociation, ValidationMode,
    },
    component_revision::{REVISION_LIMITS, RetainedComponentSnapshot},
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
    value_contract::strict_json,
};

const MAX_REVIEW_RECORDS: usize = 256;
const MAX_REVIEW_BYTES: i64 = 67_108_864;

#[derive(thiserror::Error)]
pub enum AssociationReviewStoreError {
    #[error("association review database operation failed")]
    Database(#[source] PgError),
    #[error("association review selected revision read failed")]
    Revisions(#[source] RevisionStoreError),
    #[error("association review record integrity failed")]
    Integrity,
    #[error("association review records do not cover the selected combination")]
    Combination,
    #[error("association review evidence is missing, failed or inconsistent")]
    Evidence,
    #[error("association review selection requires repeatable-read or serializable isolation")]
    Isolation,
    #[error("association review exceeds technical transport capacity")]
    Capacity,
    #[error("system-seed review provenance requires its bootstrap evidence adapter")]
    UnsupportedSystemSeed,
}
impl std::fmt::Debug for AssociationReviewStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_string())
    }
}
fn database(error: tokio_postgres::Error) -> AssociationReviewStoreError {
    AssociationReviewStoreError::Database(error.into())
}
fn digest(bytes: &str) -> String {
    format!("{:x}", Sha256::digest(bytes.as_bytes()))
}
fn checksum_text(checksum: [u8; 32]) -> String {
    checksum.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn reference(text: &str) -> Result<Uuid, AssociationReviewStoreError> {
    let id = Uuid::parse_str(text).map_err(|_| AssociationReviewStoreError::Evidence)?;
    if id.is_nil() || id.to_string() != text {
        return Err(AssociationReviewStoreError::Evidence);
    }
    Ok(id)
}

/// Retains the actual records needed by the catalogue's provenance verifier.
/// Private fields prevent callers from manufacturing a successful read result.
/// A resolved record is not proof that its required producer is wired, semantic
/// approval, an active catalogue entry, an invocation grant or a task manifest.
pub struct RetainedAssociationReviewRecords {
    approval: AssociationApprovalDeclaration,
    evidence: BTreeMap<Uuid, String>,
    snapshot: RetainedComponentSnapshot,
}
impl RetainedAssociationReviewRecords {
    pub fn approval(&self) -> &AssociationApprovalDeclaration {
        &self.approval
    }
    pub fn snapshot(&self) -> &RetainedComponentSnapshot {
        &self.snapshot
    }
    /// Sensitive audit bytes; an authorized supervisor may retain them with
    /// selection evidence. They must not be copied into model-visible state.
    pub fn evidence(&self) -> &BTreeMap<Uuid, String> {
        &self.evidence
    }
}

/// Read from the same database snapshot as matching and exact revision reads.
/// No transaction creation/commit, fallback, newest lookup or approval writer.
pub async fn retain_authored_association_reviews(
    tx: &Transaction<'_>,
    approval_id: Uuid,
    association: &SkillAssociation,
    selected: &[ComponentRevisionRef],
) -> Result<RetainedAssociationReviewRecords, AssociationReviewStoreError> {
    if approval_id.is_nil() {
        return Err(AssociationReviewStoreError::Integrity);
    }
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(database)?
        .get(0);
    if !coherent {
        return Err(AssociationReviewStoreError::Isolation);
    }
    let row = tx.query_opt(
        "SELECT approval_bytes, checksum FROM reborn_skill_association_approvals WHERE approval_id=$1",
        &[&approval_id],
    ).await.map_err(database)?.ok_or(AssociationReviewStoreError::Evidence)?;
    let bytes: String = row.get(0);
    if digest(&bytes) != row.get::<_, String>(1) {
        return Err(AssociationReviewStoreError::Integrity);
    }
    let approval = AssociationApprovalDeclaration::from_json(&bytes, REVISION_LIMITS)
        .map_err(|_| AssociationReviewStoreError::Integrity)?;
    if approval.approval_id() != approval_id {
        return Err(AssociationReviewStoreError::Integrity);
    }
    approval
        .require_selected_combination(association, selected)
        .map_err(|_| AssociationReviewStoreError::Combination)?;
    // Equal reference strings alone are insufficient. Read the complete usage
    // closure from the actual immutable revision store in this same view, and
    // verify that these are the association bytes owned by the selected Skill.
    let snapshot = PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[association.skill_uuid()],
        selected,
    )
    .await
    .map_err(AssociationReviewStoreError::Revisions)?;
    if snapshot.revisions()[&association.skill_uuid()]
        .draft()
        .association()
        .is_none_or(|actual| actual.exact_bytes() != association.exact_bytes())
    {
        return Err(AssociationReviewStoreError::Combination);
    }
    if approval.mode() != ValidationMode::Authored {
        return Err(AssociationReviewStoreError::UnsupportedSystemSeed);
    }
    // An evidence record cannot satisfy two stages, nor may a behavior record
    // appear twice to inflate acceptance. Review the same exact set in Q2.
    let q1 = reference(approval.q1_reference())?;
    let q2 = reference(
        approval
            .q2_reference()
            .ok_or(AssociationReviewStoreError::Evidence)?,
    )?;
    let mut expected = BTreeMap::from([(q1, "q1"), (q2, "human_q2")]);
    if expected.len() != 2 {
        return Err(AssociationReviewStoreError::Evidence);
    }
    if approval.behavioral_references().len() + 2 > MAX_REVIEW_RECORDS {
        return Err(AssociationReviewStoreError::Capacity);
    }
    let mut reviewed = BTreeSet::from([q1]);
    for behavior in approval.behavioral_references() {
        let id = reference(behavior)?;
        if expected.insert(id, "behavior").is_some() {
            return Err(AssociationReviewStoreError::Evidence);
        }
        reviewed.insert(id);
    }
    let ids: Vec<_> = expected.keys().copied().collect();
    let totals = tx
        .query_one(
            "SELECT count(*), COALESCE(sum(octet_length(evidence_bytes)),0)::bigint
         FROM reborn_component_review_evidence WHERE evidence_id=ANY($1::uuid[])",
            &[&ids],
        )
        .await
        .map_err(database)?;
    if totals.get::<_, i64>(0) != ids.len() as i64 {
        return Err(AssociationReviewStoreError::Evidence);
    }
    if totals.get::<_, i64>(1) > MAX_REVIEW_BYTES {
        return Err(AssociationReviewStoreError::Capacity);
    }
    let rows = tx
        .query(
            "SELECT evidence_id,evidence_bytes,checksum FROM reborn_component_review_evidence
         WHERE evidence_id=ANY($1::uuid[]) ORDER BY evidence_id",
            &[&ids],
        )
        .await
        .map_err(database)?;
    let mut evidence = BTreeMap::new();
    let selected_components: BTreeMap<_, _> = selected
        .iter()
        .map(|r| {
            (
                r.uuid,
                (
                    i64::from(r.class_code),
                    r.version,
                    checksum_text(r.checksum),
                ),
            )
        })
        .collect();
    let association_checksum = digest(association.exact_bytes());
    for row in rows {
        let id: Uuid = row.get(0);
        let bytes: String = row.get(1);
        if digest(&bytes) != row.get::<_, String>(2) {
            return Err(AssociationReviewStoreError::Integrity);
        }
        let value = strict_json(&bytes, REVISION_LIMITS)
            .map_err(|_| AssociationReviewStoreError::Integrity)?;
        validate_evidence(
            &value,
            id,
            expected[&id],
            &association_checksum,
            &selected_components,
            if id == q2 { Some(&reviewed) } else { None },
        )?;
        evidence.insert(id, bytes);
    }
    Ok(RetainedAssociationReviewRecords {
        approval,
        evidence,
        snapshot,
    })
}

fn validate_evidence(
    value: &Value,
    id: Uuid,
    kind: &str,
    association_checksum: &str,
    expected: &BTreeMap<Uuid, (i64, u64, String)>,
    reviewed: Option<&BTreeSet<Uuid>>,
) -> Result<(), AssociationReviewStoreError> {
    let fail = || AssociationReviewStoreError::Evidence;
    let object = value.as_object().ok_or_else(fail)?;
    let fields = [
        "format",
        "evidence_id",
        "kind",
        "validation_mode",
        "association_checksum",
        "components",
        "succeeded",
        "report",
        "reviewed_evidence",
    ];
    if object.len() != fields.len()
        || fields.iter().any(|key| !object.contains_key(*key))
        || object["format"] != "component-review-evidence/1"
        || object["evidence_id"] != id.to_string()
        || object["kind"] != kind
        || object["validation_mode"] != "authored"
        || object["association_checksum"] != association_checksum
        || object["succeeded"] != true
        || !object["report"].is_object()
    {
        return Err(fail());
    }
    let components = object["components"].as_array().ok_or_else(fail)?;
    let mut found = BTreeSet::new();
    if components.len() != expected.len() {
        return Err(fail());
    }
    for component in components {
        let c = component.as_object().ok_or_else(fail)?;
        let uuid = reference(c.get("uuid").and_then(Value::as_str).ok_or_else(fail)?)?;
        let exact = expected.get(&uuid).ok_or_else(fail)?;
        if c.len() != 4
            || !found.insert(uuid)
            || c.get("class_code").and_then(Value::as_i64) != Some(exact.0)
            || c.get("version").and_then(Value::as_u64) != Some(exact.1)
            || c.get("checksum").and_then(Value::as_str) != Some(exact.2.as_str())
        {
            return Err(fail());
        }
    }
    let mut actual_reviewed = BTreeSet::new();
    for item in object["reviewed_evidence"].as_array().ok_or_else(fail)? {
        let item = reference(item.as_str().ok_or_else(fail)?)?;
        if !actual_reviewed.insert(item) {
            return Err(fail());
        }
    }
    if reviewed.is_some_and(|expected| expected != &actual_reviewed)
        || (reviewed.is_none() && !actual_reviewed.is_empty())
    {
        return Err(fail());
    }
    Ok(())
}
