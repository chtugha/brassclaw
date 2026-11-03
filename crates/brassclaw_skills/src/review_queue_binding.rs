//! Immutable original queue/subject seals. No work claim, approval or Tool grant.
use super::{Reference, RetainedReviewSubmission, SubmissionStoreError, database, hex};
use crate::{
    association_contract::ComponentRevisionRef, component_revision::REVISION_LIMITS,
    value_contract::strict_json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    format: String,
    queue_id: Uuid,
    submission_id: Uuid,
    subject_checksum: String,
    candidate: Reference,
    base: Option<Reference>,
    component_bytes: String,
    queue_bytes: String,
    reviewed_bytes: String,
}

/// Private construction requires immutable readback and complete subject checks.
/// Historical reads work after Q2 removes the mutable queue.
pub struct RetainedQueueReview {
    binding: Binding,
    bytes: String,
    checksum: String,
}
impl RetainedQueueReview {
    pub fn submission_id(&self) -> Uuid {
        self.binding.submission_id
    }
    pub fn queue_id(&self) -> Uuid {
        self.binding.queue_id
    }
    pub fn subject_checksum(&self) -> &str {
        &self.binding.subject_checksum
    }
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
    pub fn component_bytes(&self) -> &str {
        &self.binding.component_bytes
    }
    pub fn queue_bytes(&self) -> &str {
        &self.binding.queue_bytes
    }
    pub fn candidate(&self) -> Result<ComponentRevisionRef, SubmissionStoreError> {
        self.binding.candidate.parsed()
    }
    pub fn require_subject(
        &self,
        subject: &super::RetainedValidatorSubject,
    ) -> Result<(), SubmissionStoreError> {
        if self.submission_id() != subject.submission_id()
            || self.subject_checksum() != subject.subject_checksum()
        {
            return Err(SubmissionStoreError::Conflict);
        }
        Ok(())
    }
}
fn root(
    subject: &RetainedReviewSubmission,
    component: Uuid,
) -> Result<(ComponentRevisionRef, Option<ComponentRevisionRef>), SubmissionStoreError> {
    if subject.candidate.uuid == component {
        return Ok((subject.candidate, subject.base));
    }
    subject
        .function_partner
        .filter(|p| p.changed && p.reference.uuid == component)
        .map(|p| (p.reference, p.base))
        .ok_or(SubmissionStoreError::Invalid)
}

/// Retain only captures taken under queue-before-component locks in this same
/// author transaction. The DB independently compares the real subject and rows.
pub async fn retain_in_transaction(
    tx: &Transaction<'_>,
    subject: &RetainedReviewSubmission,
    queue_id: Uuid,
    component: Uuid,
    component_bytes: String,
    queue_bytes: String,
    reviewed_bytes: String,
) -> Result<RetainedQueueReview, SubmissionStoreError> {
    let (candidate, base) = root(subject, component)?;
    let binding = Binding {
        format: "component-review-queue-binding/1".into(),
        queue_id,
        submission_id: subject.id,
        subject_checksum: subject.checksum.clone(),
        candidate: candidate.into(),
        base: base.map(Into::into),
        component_bytes,
        queue_bytes,
        reviewed_bytes,
    };
    let bytes = serde_json::to_string(&binding).map_err(|_| SubmissionStoreError::Invalid)?;
    let checksum = hex(Sha256::digest(bytes.as_bytes()).into());
    if let Some(existing) = read_in_transaction(tx, queue_id).await? {
        if existing.bytes != bytes || existing.checksum != checksum {
            return Err(SubmissionStoreError::Conflict);
        }
        return Ok(existing);
    }
    tx.execute("INSERT INTO reborn_component_review_queue_bindings(queue_id,submission_id,component_id,binding_bytes,checksum) VALUES($1,$2,$3,$4,$5)",
        &[&queue_id,&subject.id,&component,&bytes,&checksum]).await.map_err(database)?;
    let retained = read_in_transaction(tx, queue_id)
        .await?
        .ok_or(SubmissionStoreError::Invalid)?;
    if retained.bytes != bytes || retained.checksum != checksum {
        return Err(SubmissionStoreError::Invalid);
    }
    Ok(retained)
}

/// Read the original operation, including after queue removal. Current-state
/// CAS is intentionally separate and mandatory for every queue mutation.
pub async fn read_in_transaction(
    tx: &Transaction<'_>,
    queue_id: Uuid,
) -> Result<Option<RetainedQueueReview>, SubmissionStoreError> {
    super::require_snapshot(tx).await?;
    let Some(row)=tx.query_opt("SELECT submission_id,component_id,binding_bytes,checksum FROM reborn_component_review_queue_bindings WHERE queue_id=$1",&[&queue_id]).await.map_err(database)? else {return Ok(None)};
    let bytes: String = row.get(2);
    let checksum: String = row.get(3);
    let binding: Binding = serde_json::from_value(
        strict_json(
            &bytes,
            crate::value_contract::ContractLimits {
                max_bytes: 67_108_864,
                ..REVISION_LIMITS
            },
        )
        .map_err(|_| SubmissionStoreError::Invalid)?,
    )
    .map_err(|_| SubmissionStoreError::Invalid)?;
    let subject = super::PgReviewSubmissionStore::read_in_transaction(tx, binding.submission_id)
        .await?
        .ok_or(SubmissionStoreError::Invalid)?;
    let (candidate, base) = root(&subject, row.get(1))?;
    let expected = if subject.function_partner.is_some_and(|p| p.changed) {
        2_i64
    } else {
        1_i64
    };
    let request=tx.query_opt("SELECT queue_ids, (SELECT count(*) FROM reborn_component_review_queue_bindings WHERE submission_id=$1) FROM reborn_component_review_queue_requests WHERE submission_id=$1",&[&binding.submission_id]).await.map_err(database)?
        .ok_or(SubmissionStoreError::Invalid)?;
    let targets: Vec<Uuid> = request.get(0);
    // During an author transaction the second changed-root binding is not yet
    // inserted. Final callers require the complete set through the deferred DB
    // constraint at commit; this reader still verifies exact original membership.
    if !targets.contains(&queue_id)
        || targets.len() as i64 != expected
        || request.get::<_, i64>(1) > expected
    {
        return Err(SubmissionStoreError::Invalid);
    }

    if serde_json::to_string(&binding).map_err(|_| SubmissionStoreError::Invalid)? != bytes
        || binding.format != "component-review-queue-binding/1"
        || binding.queue_id != queue_id
        || binding.submission_id != row.get::<_, Uuid>(0)
        || binding.subject_checksum != subject.checksum
        || binding.candidate.parsed()? != candidate
        || binding.base.as_ref().map(Reference::parsed).transpose()? != base
        || checksum != hex(Sha256::digest(bytes.as_bytes()).into())
    {
        return Err(SubmissionStoreError::Invalid);
    }
    // Recheck the projection against actual retained revision bytes, not JSON
    // supplied by the binding caller. All remaining v3 metadata stays in subject.
    let document = subject.snapshot.revisions()[&candidate.uuid]
        .draft()
        .document();
    let class = i16::try_from(candidate.class_code).map_err(|_| SubmissionStoreError::Invalid)?;
    let row=tx.query_one("SELECT $2::jsonb ?& review_queue_fields($1::smallint) AND review_queue_projection($1::smallint,$2::jsonb)=review_queue_projection($1::smallint,$3::jsonb) AND NOT EXISTS(SELECT 1 FROM jsonb_each($2::jsonb) d JOIN jsonb_each($3::jsonb) e ON d.key=e.key WHERE d.value IS DISTINCT FROM e.value)",
        &[&class,document,&serde_json::from_str::<serde_json::Value>(&binding.reviewed_bytes).map_err(|_|SubmissionStoreError::Invalid)?]).await.map_err(database)?;
    if row.get::<_, Option<bool>>(0) != Some(true) {
        return Err(SubmissionStoreError::Invalid);
    }
    Ok(Some(RetainedQueueReview {
        binding,
        bytes,
        checksum,
    }))
}
