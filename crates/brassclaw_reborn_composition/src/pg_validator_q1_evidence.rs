//! Durable Q1 evidence writer for completed validator Recipe executions.
//!
//! This is the trusted bridge from a completed `RetainedRecipeExecution` (with
//! an `ObservedSubjectValidatorChecks` result) to a persisted immutable
//! `component-review-evidence/1` row in `reborn_component_review_evidence` (V102)
//! plus a corresponding index row in `reborn_validator_q1_runs` (V202).
//!
//! # Authority chain
//!
//! The caller must supply all of the following sealed inputs; none come from
//! request data, caller-supplied JSON or a public API endpoint:
//!
//! - A `RetainedValidatorSubject` constructed from the sealed submission store.
//! - A `RetainedValidatorSelection` constructed from the sealed selection store.
//! - An `ObservedSubjectValidatorChecks` built by the sealed executor observer.
//! - The `task_handle` UUID from the completed task — not a caller-supplied ID.
//!
//! Storing this evidence does not establish Q1, combination approval, semantic
//! agreement, human Q2 or activation. Those remain separate gates.
//!
//! # Idempotency
//!
//! The evidence ID is deterministically derived from the submission ID and
//! validator selection checksum. An identical retry with the same sealed inputs
//! is a no-op when the stored row is byte-identical; differing bytes are an
//! integrity error that must not be silently ignored. The run row in
//! `reborn_validator_q1_runs` is inserted idempotently under the same natural
//! key (submission_id, validator_selection_checksum).
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

#![forbid(unsafe_code)]

use brassclaw_engine::memory::retained_validator::ObservedSubjectValidatorChecks;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{RetainedValidatorSubject, selection::RetainedValidatorSelection},
    value_contract::validate_data_bounds,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

const MAX_EVIDENCE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ValidatorQ1EvidenceError {
    #[error("validator Q1 evidence: {0}")]
    Invalid(&'static str),
    #[error("validator Q1 evidence database operation failed")]
    Database(#[source] tokio_postgres::Error),
    #[error("validator Q1 evidence database connection failed")]
    Connection(#[source] deadpool_postgres::PoolError),
    #[error("validator Q1 evidence integrity check failed")]
    Integrity,
    #[error("original validator observation I/O deadline elapsed; outcome unresolved")]
    Unresolved,
}
fn invalid(reason: &'static str) -> ValidatorQ1EvidenceError {
    ValidatorQ1EvidenceError::Invalid(reason)
}
fn db(error: tokio_postgres::Error) -> ValidatorQ1EvidenceError {
    ValidatorQ1EvidenceError::Database(error)
}

#[path = "pg_validator_q1_verdict.rs"]
mod verdict;
pub(crate) use verdict::read_delivered_validator_q1;
#[cfg(test)]
pub(crate) use verdict::read_settled_validator_q1;

/// Opaque reference to a persisted Q1 evidence record. The UUID identifies
/// the immutable row; bytes are sealed inside and never exposed to callers.
///
/// This reference may be used as the `q1_ref` in a later
/// `PureComponentApprovalDeclaration` or `skill-association-approval/1`, but
/// only after the complete trusted review path is satisfied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PersistedValidatorQ1Evidence {
    evidence_id: Uuid,
}
impl PersistedValidatorQ1Evidence {
    #[allow(dead_code)] // used by approval path after Step 12
    pub(crate) fn evidence_id(&self) -> Uuid {
        self.evidence_id
    }
}

/// Persist one Q1 evidence record for a completed single-class validator execution.
///
/// The evidence record is `component-review-evidence/1` format and is stored in
/// `reborn_component_review_evidence` (V102). It contains:
/// - The submission ID and subject checksum (sealed subject identity).
/// - The validator selection checksum (sealed validator identity).
/// - The class code of the reviewed component.
/// - The complete observed result report from the sealed executor observer.
/// - `succeeded: true/false` based on whether the aggregation step passed.
/// - `semantic_approval: false` — semantic review and human Q2 remain separate.
///
/// # Arguments
///
/// - `pool`: database pool.
/// - `subject`: sealed `RetainedValidatorSubject` from the submission store.
/// - `selection`: sealed `RetainedValidatorSelection` from the selection store.
/// - `checks`: sealed `ObservedSubjectValidatorChecks` from the executor observer.
/// - `task_handle`: the UUID identifying the completed task (not caller-supplied).
pub(crate) async fn persist_validator_q1_evidence(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    selection: &RetainedValidatorSelection,
    checks: &ObservedSubjectValidatorChecks,
    task_handle: Uuid,
) -> Result<PersistedValidatorQ1Evidence, ValidatorQ1EvidenceError> {
    persist_observation(pool, subject, selection, checks, task_handle, None, None).await
}
/// Source-only observations use their original acknowledged whole-value bounds,
/// not the legacy writer's fixed eight-MiB quota. They still grant no full Q1.
pub(crate) async fn persist_validator_source_evidence(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    selection: &RetainedValidatorSelection,
    checks: &ObservedSubjectValidatorChecks,
    task_handle: Uuid,
    original_limits: brassclaw_skills::value_contract::ContractLimits,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
) -> Result<PersistedValidatorQ1Evidence, ValidatorQ1EvidenceError> {
    if selection.source_qualification().is_none() {
        return Err(invalid("qualified Source selection required"));
    }
    // Capture the existing acknowledged technical store deadline once. Both
    // the single metadata write and any fresh original readback are bounded;
    // a timeout does not authorize a second write or native replay.
    let policy = storage
        .snapshot()
        .and_then(|p| p.validate())
        .map_err(|_| ValidatorQ1EvidenceError::Unresolved)?;
    persist_observation(
        pool,
        subject,
        selection,
        checks,
        task_handle,
        Some(original_limits),
        Some(policy.timeout),
    )
    .await
}
async fn persist_observation(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    selection: &RetainedValidatorSelection,
    checks: &ObservedSubjectValidatorChecks,
    task_handle: Uuid,
    original_limits: Option<brassclaw_skills::value_contract::ContractLimits>,
    deadline: Option<std::time::Duration>,
) -> Result<PersistedValidatorQ1Evidence, ValidatorQ1EvidenceError> {
    if task_handle.is_nil() {
        return Err(invalid("task handle must be a non-nil UUID"));
    }
    // Verify subject/selection agreement before storing any evidence.
    if selection.submission_id() != subject.submission_id()
        || selection.subject_checksum() != subject.subject_checksum()
    {
        return Err(invalid(
            "validator selection does not match the review subject",
        ));
    }

    // Derive a deterministic evidence ID from the submission and validator
    // selection so idempotent retries recover the same row without re-running
    // the validator Recipe. Mixing both IDs prevents collisions across subjects
    // and across different validators used for the same subject.
    let id_bytes = {
        let mut hasher = Sha256::new();
        hasher.update(b"validator-q1-evidence/1:");
        hasher.update(subject.submission_id().as_bytes());
        hasher.update(b":validator:");
        hasher.update(Sha256::digest(selection.validator_bytes().as_bytes()).as_slice());
        let hash: [u8; 32] = hasher.finalize().into();
        // Build a deterministic UUID v5-like from the hash bytes.
        // We use the first 16 bytes, setting version=5 and variant bits per RFC.
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes.copy_from_slice(&hash[..16]);
        uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50; // version 5
        uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80; // variant RFC 4122
        uuid_bytes
    };
    let evidence_id = Uuid::from_bytes(id_bytes);

    // Extract the succeeded flag from the sealed observation report. The report
    // format is `observed-subject-validator-checks/1` containing `aggregation`.
    let report = checks.report();
    let succeeded = report["aggregation"]["pass"]
        .as_bool()
        .ok_or_else(|| invalid("observation report is missing required aggregation.pass field"))?;

    // Build the evidence record.
    let validator_selection_checksum: String =
        Sha256::digest(selection.validator_bytes().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
    let record = json!({
        "format": "component-review-evidence/1",
        "evidence_id": evidence_id,
        "kind": "q1",
        "validation_mode": "authored",
        "submission_id": subject.submission_id(),
        "subject_checksum": subject.subject_checksum(),
        "validator_selection_checksum": validator_selection_checksum,
        "class_code": subject.candidate().class_code,
        "task_handle": task_handle,
        "succeeded": succeeded,
        "reviewed_evidence": [],
        "report": report,
    });

    validate_data_bounds(&record, original_limits.unwrap_or(REVISION_LIMITS))
        .map_err(|_| invalid("Q1 evidence record exceeds technical data contract limits"))?;
    let bytes = record.to_string();
    if bytes.len() > original_limits.map_or(MAX_EVIDENCE_BYTES, |limits| limits.max_bytes) {
        return Err(invalid(
            "serialized Q1 evidence record exceeds technical capacity",
        ));
    }
    let checksum: String = Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    let class_i16 = i16::try_from(subject.candidate().class_code)
        .map_err(|_| invalid("candidate class code is out of i16 range"))?;
    let original = ObservationOriginal {
        evidence_id,
        submission_id: subject.submission_id(),
        selection_checksum: validator_selection_checksum,
        class_code: class_i16,
        task_handle,
        succeeded,
        bytes,
        checksum,
    };
    // At most one write transaction. A failed/lost response is not permission
    // to repeat the metadata write, native parser, task or execution identity.
    if let Err(primary) = within_deadline(deadline, original.write(pool)).await {
        // write() has ended and its unacknowledged connection has been
        // physically discarded before this fresh, read-only RR checkout.
        match within_deadline(deadline, original.read(pool)).await {
            Ok(true) => {}
            Ok(false) => return Err(primary),
            Err(error) => return Err(error),
        }
    }
    Ok(PersistedValidatorQ1Evidence { evidence_id })
}

/// Exact metadata derived solely from the original sealed execution observer.
/// This is not a constructor for observations or any live native authority.
struct ObservationOriginal {
    evidence_id: Uuid,
    submission_id: Uuid,
    selection_checksum: String,
    class_code: i16,
    task_handle: Uuid,
    succeeded: bool,
    bytes: String,
    checksum: String,
}
impl ObservationOriginal {
    async fn require_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<bool, ValidatorQ1EvidenceError> {
        let evidence = tx.query_opt("SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1", &[&self.evidence_id]).await.map_err(db)?;
        let run = tx.query_opt("SELECT run_id,submission_id,validator_selection_checksum,candidate_class_code,task_handle,evidence_id,succeeded FROM reborn_validator_q1_runs WHERE submission_id=$1 AND validator_selection_checksum=$2", &[&self.submission_id,&self.selection_checksum]).await.map_err(db)?;
        let (evidence, run) = match (evidence, run) {
            (None, None) => return Ok(false),
            (Some(evidence), Some(run)) => (evidence, run),
            _ => return Err(ValidatorQ1EvidenceError::Integrity),
        };
        if evidence.get::<_, &str>(0) != self.bytes
            || evidence.get::<_, &str>(1) != self.checksum
            || run.get::<_, Uuid>(0) != self.evidence_id
            || run.get::<_, Uuid>(1) != self.submission_id
            || run.get::<_, &str>(2) != self.selection_checksum
            || run.get::<_, i16>(3) != self.class_code
            || run.get::<_, Uuid>(4) != self.task_handle
            || run.get::<_, Uuid>(5) != self.evidence_id
            || run.get::<_, bool>(6) != self.succeeded
        {
            return Err(ValidatorQ1EvidenceError::Integrity);
        }
        Ok(true)
    }
    async fn write(&self, pool: &PgPool) -> Result<(), ValidatorQ1EvidenceError> {
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            pool.get()
                .await
                .map_err(ValidatorQ1EvidenceError::Connection)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(db)?;
        tx.execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING", &[&self.evidence_id,&self.bytes,&self.checksum]).await.map_err(db)?;
        tx.execute("INSERT INTO reborn_validator_q1_runs(run_id,submission_id,validator_selection_checksum,candidate_class_code,task_handle,evidence_id,succeeded) VALUES($1,$2,$3,$4,$5,$1,$6) ON CONFLICT(submission_id,validator_selection_checksum) DO NOTHING", &[&self.evidence_id,&self.submission_id,&self.selection_checksum,&self.class_code,&self.task_handle,&self.succeeded]).await.map_err(db)?;
        if !self.require_in_transaction(&tx).await? {
            return Err(ValidatorQ1EvidenceError::Integrity);
        }
        tx.commit().await.map_err(db)?;
        client.recycle_after_commit();
        Ok(())
    }
    async fn read(&self, pool: &PgPool) -> Result<bool, ValidatorQ1EvidenceError> {
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            pool.get()
                .await
                .map_err(ValidatorQ1EvidenceError::Connection)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(db)?;
        let exact = self.require_in_transaction(&tx).await?;
        tx.commit().await.map_err(db)?;
        client.recycle_after_commit();
        Ok(exact)
    }
}
async fn within_deadline<T>(
    deadline: Option<std::time::Duration>,
    operation: impl std::future::Future<Output = Result<T, ValidatorQ1EvidenceError>>,
) -> Result<T, ValidatorQ1EvidenceError> {
    match deadline {
        Some(deadline) => tokio::time::timeout(deadline, operation)
            .await
            .map_err(|_| ValidatorQ1EvidenceError::Unresolved)?,
        None => operation.await,
    }
}

#[cfg(all(test, feature = "skills-db"))]
mod tests {
    use super::*;

    #[test]
    fn evidence_id_is_deterministic() {
        // The same submission_id and validator_bytes always produce the same ID.
        let submission_id = Uuid::new_v4();
        let validator_bytes = r#"{"format":"monty-retained-recipe-selection/1"}"#;

        let id_bytes1 = {
            let mut hasher = Sha256::new();
            hasher.update(b"validator-q1-evidence/1:");
            hasher.update(submission_id.as_bytes());
            hasher.update(b":validator:");
            hasher.update(Sha256::digest(validator_bytes.as_bytes()).as_slice());
            let hash: [u8; 32] = hasher.finalize().into();
            let mut uuid_bytes = [0u8; 16];
            uuid_bytes.copy_from_slice(&hash[..16]);
            uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
            uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
            uuid_bytes
        };
        let id_bytes2 = {
            let mut hasher = Sha256::new();
            hasher.update(b"validator-q1-evidence/1:");
            hasher.update(submission_id.as_bytes());
            hasher.update(b":validator:");
            hasher.update(Sha256::digest(validator_bytes.as_bytes()).as_slice());
            let hash: [u8; 32] = hasher.finalize().into();
            let mut uuid_bytes = [0u8; 16];
            uuid_bytes.copy_from_slice(&hash[..16]);
            uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
            uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
            uuid_bytes
        };
        assert_eq!(id_bytes1, id_bytes2);
        let uuid1 = Uuid::from_bytes(id_bytes1);
        assert!(!uuid1.is_nil());
    }

    #[test]
    fn different_subjects_produce_different_ids() {
        let sub1 = Uuid::new_v4();
        let sub2 = Uuid::new_v4();
        let validator_bytes = r#"{"format":"monty-retained-recipe-selection/1"}"#;
        assert_ne!(sub1, sub2);
        // Just verify the IDs come from the subject IDs — different inputs differ.
        let derive = |sub: Uuid| {
            let mut hasher = Sha256::new();
            hasher.update(b"validator-q1-evidence/1:");
            hasher.update(sub.as_bytes());
            hasher.update(b":validator:");
            hasher.update(Sha256::digest(validator_bytes.as_bytes()).as_slice());
            let hash: [u8; 32] = hasher.finalize().into();
            let mut uuid_bytes = [0u8; 16];
            uuid_bytes.copy_from_slice(&hash[..16]);
            uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
            uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
            Uuid::from_bytes(uuid_bytes)
        };
        assert_ne!(derive(sub1), derive(sub2));
    }
}
