//! Exact immutable association/review resolution and human-Q2 recording.
//! Trusted Q1/behavior producers supply actual evidence; the authenticated
//! human ingress owns decisions. No arbitrary evidence-write API, catalogue
//! activation or Tool authority is supplied. Legacy labels/receipts do not
//! substitute for exact reviewed combinations or actual validation evidence.

use std::collections::{BTreeMap, BTreeSet};

use brassclaw_pg::PgError;
use serde_json::{Value, json};
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

/// Exact pre-Q2 view. Reading it neither approves nor activates anything.
/// Only trusted Q1/behavior producers may write the referenced evidence table.
/// These privately retained fields bind the decision to actual stored bytes.
pub struct PreparedHumanAssociationReview {
    association: SkillAssociation,
    snapshot: RetainedComponentSnapshot,
    q1: Uuid,
    behavioral: BTreeSet<Uuid>,
    evidence: BTreeMap<Uuid, String>,
    view: Value,
    checksum: String,
}
impl PreparedHumanAssociationReview {
    pub fn view(&self) -> &Value {
        &self.view
    }
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}

/// Use the same coherent snapshot as the subsequent Q2 write. The caller names
/// evidence, never supplies source, versions, reports or a successful-Q1 flag.
/// Draft heads/newer replacements do not change the exact combination reviewed.
pub async fn prepare_human_association_review(
    tx: &Transaction<'_>,
    skill: Uuid,
    q1: Uuid,
    behavioral: &[Uuid],
) -> Result<PreparedHumanAssociationReview, AssociationReviewStoreError> {
    if skill.is_nil() || q1.is_nil() || behavioral.is_empty() {
        return Err(AssociationReviewStoreError::Evidence);
    }
    let behaviors: BTreeSet<_> = behavioral.iter().copied().collect();
    if behaviors.len() != behavioral.len()
        || behaviors.contains(&q1)
        || behaviors.iter().any(Uuid::is_nil)
    {
        return Err(AssociationReviewStoreError::Evidence);
    }
    if behaviors.len() + 2 > MAX_REVIEW_RECORDS {
        return Err(AssociationReviewStoreError::Capacity);
    }
    let ids: Vec<_> = std::iter::once(q1)
        .chain(behaviors.iter().copied())
        .collect();
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
    let mut parsed = BTreeMap::new();
    for row in rows {
        let id: Uuid = row.get(0);
        let bytes: String = row.get(1);
        if digest(&bytes) != row.get::<_, String>(2) {
            return Err(AssociationReviewStoreError::Integrity);
        }
        let value = strict_json(&bytes, REVISION_LIMITS)
            .map_err(|_| AssociationReviewStoreError::Integrity)?;
        evidence.insert(id, bytes);
        parsed.insert(id, value);
    }
    // Parse the selected references with the existing exact declaration parser.
    // This temporary declaration is not persisted and does not create approval.
    let temporary = json!({
        "format":"skill-association-approval/1", "approval_id":q1,
        "association_checksum":parsed[&q1]["association_checksum"],
        "components":parsed[&q1]["components"], "validation_mode":"authored",
        "q1_ref":q1, "q2_ref":q1, "behavioral_refs":behavioral,
    });
    let declaration =
        AssociationApprovalDeclaration::from_json(&temporary.to_string(), REVISION_LIMITS)
            .map_err(|_| AssociationReviewStoreError::Integrity)?;
    let selected: Vec<_> = declaration.components().values().copied().collect();
    let snapshot = PgComponentRevisionStore::read_exact_in_transaction(tx, &[skill], &selected)
        .await
        .map_err(AssociationReviewStoreError::Revisions)?;
    let association = snapshot
        .revisions()
        .get(&skill)
        .and_then(|r| r.draft().association())
        .cloned()
        .ok_or(AssociationReviewStoreError::Combination)?;
    declaration
        .require_selected_combination(&association, &selected)
        .map_err(|_| AssociationReviewStoreError::Combination)?;
    let expected: BTreeMap<_, _> = selected
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
    for (id, value) in &parsed {
        validate_evidence(
            value,
            *id,
            if *id == q1 { "q1" } else { "behavior" },
            &digest(association.exact_bytes()),
            &expected,
            None,
        )?;
    }
    let components: Vec<_> = snapshot
        .revisions()
        .values()
        .map(|r| {
            let reference = r.reference();
            json!({"uuid":reference.uuid,"class_code":reference.class_code,
            "version":reference.version,"checksum":checksum_text(reference.checksum),
            "revision_bytes":r.draft().exact_bytes()})
        })
        .collect();
    let records: Vec<_> = evidence
        .iter()
        .map(
            |(id, bytes)| json!({"evidence_id":id,"checksum":digest(bytes),"evidence_bytes":bytes}),
        )
        .collect();
    let view = json!({"format":"human-association-review/1", "skill_uuid":skill,
        "association_bytes":association.exact_bytes(),"components":components,
        "evidence":records,"q1_ref":q1,"behavioral_refs":behaviors,
        "scope":"one-tool-usage","activates_catalogue":false,"grants_tool_permission":false});
    let bytes = view.to_string();
    if bytes.len() > MAX_REVIEW_BYTES as usize {
        return Err(AssociationReviewStoreError::Capacity);
    }
    let mut hasher = Sha256::new();
    hasher.update(b"human-association-review/1\0");
    hasher.update(bytes.as_bytes());
    let checksum = format!("{:x}", hasher.finalize());
    Ok(PreparedHumanAssociationReview {
        association,
        snapshot,
        q1,
        behavioral: behaviors,
        evidence,
        view,
        checksum,
    })
}

/// Decision supplied only by an authenticated human-review ingress owner.
/// Identity comes from authentication; no model, Recipe or authoring JSON may
/// supply it. The checksum is the precise displayed view, including evidence.
pub struct HumanAssociationDecision<'a> {
    pub approval_id: Uuid,
    pub q2_id: Uuid,
    pub actor: &'a str,
    pub semantic_review: &'a str,
    pub reviewed_checksum: &'a str,
}

/// Atomically records human Q2 plus exact combination approval. Caller commits
/// and retries identical decision IDs/bytes on an ambiguous commit. No mutable
/// label, activation, Tool permission, or successful workflow is manufactured.
pub async fn record_human_association_approval(
    tx: &Transaction<'_>,
    review: &PreparedHumanAssociationReview,
    decision: HumanAssociationDecision<'_>,
) -> Result<(), AssociationReviewStoreError> {
    let note = decision.semantic_review;
    if decision.approval_id.is_nil()
        || decision.q2_id.is_nil()
        || decision.approval_id == decision.q2_id
        || review.evidence.contains_key(&decision.approval_id)
        || review.evidence.contains_key(&decision.q2_id)
        || decision.actor.trim().is_empty()
        || decision.actor.len() > 2048
        || note.trim().is_empty()
        || note.len() > 16_384
        || decision.reviewed_checksum != review.checksum
    {
        return Err(AssociationReviewStoreError::Evidence);
    }
    let components: Vec<_> = review
        .snapshot
        .revisions()
        .values()
        .map(|r| {
            let reference = r.reference();
            json!({"uuid":reference.uuid,"class_code":reference.class_code,
            "version":reference.version,"checksum":checksum_text(reference.checksum)})
        })
        .collect();
    let q2 = json!({"format":"component-review-evidence/1", "evidence_id":decision.q2_id,
        "kind":"human_q2","validation_mode":"authored",
        "association_checksum":digest(review.association.exact_bytes()),
        "components":components,"succeeded":true,
        "reviewed_evidence":review.evidence.keys().collect::<Vec<_>>(),
        "report":{"format":"human-association-decision/1", "actor":decision.actor,
            "semantic_review":note,"review_checksum":review.checksum,
            "scope":"one-tool-usage","workflow_completion":false}});
    let approval = json!({"format":"skill-association-approval/1",
        "approval_id":decision.approval_id,
        "association_checksum":digest(review.association.exact_bytes()),
        "components":components,"validation_mode":"authored",
        "q1_ref":review.q1,"q2_ref":decision.q2_id,"behavioral_refs":review.behavioral});
    let q2_bytes = q2.to_string();
    let approval_bytes = approval.to_string();
    strict_json(&q2_bytes, REVISION_LIMITS).map_err(|_| AssociationReviewStoreError::Capacity)?;
    AssociationApprovalDeclaration::from_json(&approval_bytes, REVISION_LIMITS)
        .map_err(|_| AssociationReviewStoreError::Integrity)?;
    tx.execute(
        "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
        VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING",
        &[&decision.q2_id, &q2_bytes, &digest(&q2_bytes)],
    )
    .await
    .map_err(database)?;
    let actual = tx
        .query_one(
            "SELECT evidence_bytes,checksum FROM reborn_component_review_evidence
        WHERE evidence_id=$1",
            &[&decision.q2_id],
        )
        .await
        .map_err(database)?;
    if actual.get::<_, String>(0) != q2_bytes || actual.get::<_, String>(1) != digest(&q2_bytes) {
        return Err(AssociationReviewStoreError::Integrity);
    }
    tx.execute(
        "INSERT INTO reborn_skill_association_approvals(approval_id,approval_bytes,checksum)
        VALUES($1,$2,$3) ON CONFLICT(approval_id) DO NOTHING",
        &[
            &decision.approval_id,
            &approval_bytes,
            &digest(&approval_bytes),
        ],
    )
    .await
    .map_err(database)?;
    let actual = tx
        .query_one(
            "SELECT approval_bytes,checksum FROM reborn_skill_association_approvals
        WHERE approval_id=$1",
            &[&decision.approval_id],
        )
        .await
        .map_err(database)?;
    if actual.get::<_, String>(0) != approval_bytes
        || actual.get::<_, String>(1) != digest(&approval_bytes)
    {
        return Err(AssociationReviewStoreError::Integrity);
    }
    let selected: Vec<_> = review
        .snapshot
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect();
    retain_authored_association_reviews(tx, decision.approval_id, &review.association, &selected)
        .await?;
    Ok(())
}
