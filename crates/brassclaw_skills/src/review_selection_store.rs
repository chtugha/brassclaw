//! Exact preparation metadata only. Retention supplies no approval, task claim,
//! semantic review, successful Q1, or authority to dispatch the stored workflow.
use super::{PgReviewSubmissionStore, Reference, SubmissionStoreError, database, hex};
use crate::{
    revision_store::PgComponentRevisionStore,
    value_contract::{ContractLimits, strict_json},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

/// Verified exact database preparation, not approved execution authority.
/// The engine can rebuild typed IBS from this retained graph without latest reads.
pub struct RetainedValidatorSelection {
    submission_id: Uuid,
    subject_checksum: String,
    bytes: String,
    validator_bytes: String,
    selection: serde_json::Value,
    snapshot: std::sync::Arc<crate::component_revision::RetainedComponentSnapshot>,
    qualification: Option<SourceQualification>,
    parsing_limits: ContractLimits,
}
impl RetainedValidatorSelection {
    pub fn submission_id(&self) -> Uuid {
        self.submission_id
    }
    pub fn subject_checksum(&self) -> &str {
        &self.subject_checksum
    }
    pub fn bytes(&self) -> &str {
        &self.bytes
    }
    pub fn validator_bytes(&self) -> &str {
        &self.validator_bytes
    }
    pub fn selection(&self) -> &serde_json::Value {
        &self.selection
    }
    /// The same captured policy that validated these original bytes.
    pub fn parsing_limits(&self) -> ContractLimits {
        self.parsing_limits
    }
    pub fn snapshot(&self) -> std::sync::Arc<crate::component_revision::RetainedComponentSnapshot> {
        self.snapshot.clone()
    }
    /// Untrusted qualification pins. The installation owner must authenticate
    /// their complete original proof before admission or historical acceptance.
    pub fn source_qualification(&self) -> Option<&SourceQualification> {
        self.qualification.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProofReference {
    pub id: Uuid,
    pub checksum: String,
}
/// Strict passive wire pins only. Format validation authenticates no proof,
/// component approval or execution authority; the qualified runtime owner must
/// verify the complete original native provenance in its coherent transaction.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceQualification {
    pub profile: String,
    pub workflow_proof: SourceProofReference,
    pub prerequisite_proofs: Vec<SourceProofReference>,
    pub required_checks_checksum: String,
    pub worker_checksum: String,
    /// SHA256 of the exact canonical package-context subdocument in the
    /// original F01 fixture claim, not a synthetic whole-package digest.
    pub package_checksum: String,
}
impl SourceQualification {
    pub fn validate(&self) -> Result<(), SubmissionStoreError> {
        let digest = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let mut ids = std::collections::BTreeSet::new();
        if self.profile != "pure_source_structure"
            || self.prerequisite_proofs.is_empty()
            || !digest(&self.required_checks_checksum)
            || !digest(&self.worker_checksum)
            || !digest(&self.package_checksum)
            || std::iter::once(&self.workflow_proof)
                .chain(&self.prerequisite_proofs)
                .any(|r| r.id.is_nil() || !digest(&r.checksum) || !ids.insert(r.id))
        {
            return Err(SubmissionStoreError::Invalid);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    format: String,
    submission_id: Uuid,
    subject_checksum: String,
    candidate: Reference,
    validator_selection_bytes: String,
    validator_selection_checksum: String,
    scope: String,
    #[serde(default)]
    qualification: Option<SourceQualification>,
}

async fn verify(
    tx: &Transaction<'_>,
    bytes: &str,
    limits: ContractLimits,
) -> Result<RetainedValidatorSelection, SubmissionStoreError> {
    let raw = strict_json(bytes, limits).map_err(|_| SubmissionStoreError::Invalid)?;
    let qualification_present = raw.get("qualification").is_some();
    let qualification_raw = raw.get("qualification").cloned();
    let value: Selection =
        serde_json::from_value(raw).map_err(|_| SubmissionStoreError::Invalid)?;
    let qualified = match value.format.as_str() {
        "component-validator-selection/1"
            if value.scope == "preparation-only" && !qualification_present =>
        {
            false
        }
        "component-validator-selection/2"
            if value.scope == "qualified-pure-source-prerequisite" =>
        {
            value
                .qualification
                .as_ref()
                .ok_or(SubmissionStoreError::Invalid)?
                .validate()?;
            if serde_json::to_value(&value.qualification)
                .map_err(|_| SubmissionStoreError::Invalid)?
                != qualification_raw.ok_or(SubmissionStoreError::Invalid)?
            {
                return Err(SubmissionStoreError::Invalid);
            }
            true
        }
        _ => return Err(SubmissionStoreError::Invalid),
    };
    if qualified && value.candidate.class_code != 22 {
        return Err(SubmissionStoreError::Invalid);
    }
    let subject = PgReviewSubmissionStore::read_in_transaction(tx, value.submission_id)
        .await?
        .ok_or(SubmissionStoreError::Invalid)?;
    // Existing /1 preparation covers one class/root. Never retain a partial
    // selection for a function subject with a changed or contextual partner.
    if subject.function_partner.is_some()
        || !matches!(
            subject.candidate.class_code,
            0 | 1 | 2 | 3 | 13 | 21 | 22 | 23
        )
    {
        return Err(SubmissionStoreError::Invalid);
    }
    if subject.checksum != value.subject_checksum
        || subject.candidate != value.candidate.parsed()?
        || value.validator_selection_checksum
            != hex(Sha256::digest(value.validator_selection_bytes.as_bytes()).into())
    {
        return Err(SubmissionStoreError::Invalid);
    }
    let (selection, snapshot) = verify_program_selection(
        tx,
        &value.validator_selection_bytes,
        subject.candidate.class_code,
        limits,
    )
    .await?;
    Ok(RetainedValidatorSelection {
        submission_id: value.submission_id,
        subject_checksum: value.subject_checksum,
        bytes: bytes.to_owned(),
        validator_bytes: value.validator_selection_bytes,
        selection,
        snapshot,
        qualification: value.qualification,
        parsing_limits: limits,
    })
}

pub(super) async fn verify_program_selection(
    tx: &Transaction<'_>,
    validator_bytes: &str,
    class_code: i32,
    limits: ContractLimits,
) -> Result<
    (
        serde_json::Value,
        std::sync::Arc<crate::component_revision::RetainedComponentSnapshot>,
    ),
    SubmissionStoreError,
> {
    let parsed =
        crate::retained_selection_contract::parse_selection_version(validator_bytes, limits)
            .map_err(|error| match error {
                crate::retained_selection_contract::SelectionContractError::Capacity => {
                    SubmissionStoreError::Capacity
                }
                crate::retained_selection_contract::SelectionContractError::Invalid => {
                    SubmissionStoreError::Invalid
                }
            })?;
    let root = parsed.selection().recipe();
    let refs = parsed.selection().components();
    let graph = PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[root.uuid],
        refs,
        crate::revision_store::SnapshotReadLimits::legacy_graph(),
    )
    .await?;
    parsed
        .require_snapshot(&graph)
        .map_err(|_| SubmissionStoreError::Invalid)?;
    let selection = parsed.into_value();
    let doc = graph.revisions()[&root.uuid].draft().document();
    let key = selection["variant"]["variant_key"]
        .as_str()
        .ok_or(SubmissionStoreError::Invalid)?;
    let variants = doc["variants"]
        .as_array()
        .ok_or(SubmissionStoreError::Invalid)?;
    let variant = variants
        .iter()
        .find(|variant| variant["variant_key"] == key)
        .ok_or(SubmissionStoreError::Invalid)?;
    if doc["validates_class_code"].as_i64() != Some(i64::from(class_code))
        || doc["consumer_tags"] != serde_json::json!(["05:validator"])
        || variant["intent_examples"] != serde_json::json!([])
        || selection["variant"]["intent_examples"] != serde_json::json!([])
    {
        return Err(SubmissionStoreError::Invalid);
    }
    Ok((selection, std::sync::Arc::new(graph)))
}

/// Caller retains exact bytes before I/O and acknowledges only its actual
/// successful commit. Different bytes under the original ID always conflict.
pub async fn retain_in_transaction(
    tx: &Transaction<'_>,
    bytes: &str,
    limits: ContractLimits,
) -> Result<(), SubmissionStoreError> {
    let id = verify(tx, bytes, limits).await?.submission_id;
    let checksum = hex(Sha256::digest(bytes.as_bytes()).into());
    tx.execute(
        "INSERT INTO reborn_component_validator_selections(submission_id,selection_bytes,checksum)
        VALUES($1,$2,$3) ON CONFLICT(submission_id) DO NOTHING",
        &[&id, &bytes, &checksum],
    )
    .await
    .map_err(database)?;
    let row = tx.query_one("SELECT selection_bytes,checksum FROM reborn_component_validator_selections WHERE submission_id=$1", &[&id]).await.map_err(database)?;
    if row.get::<_, String>(0) != bytes || row.get::<_, String>(1) != checksum {
        return Err(SubmissionStoreError::Conflict);
    }
    Ok(())
}

/// Reads only the original pinned bytes, never a newest validator. Full
/// preparation/approval/artifact restoration remains the execution owner's job.
pub async fn read_in_transaction(
    tx: &Transaction<'_>,
    id: Uuid,
    limits: ContractLimits,
) -> Result<Option<String>, SubmissionStoreError> {
    Ok(read_preparation_in_transaction(tx, id, limits)
        .await?
        .map(|value| value.bytes))
}

/// Sealed exact snapshot for typed IBS restoration, not a cached latest choice.
pub async fn read_preparation_in_transaction(
    tx: &Transaction<'_>,
    id: Uuid,
    limits: ContractLimits,
) -> Result<Option<RetainedValidatorSelection>, SubmissionStoreError> {
    let limits = super::read_original_preparation(tx, id)
        .await?
        .as_ref()
        .map(super::preparation_value_limits)
        .transpose()?
        .unwrap_or(limits);
    // Enforce coherent subject/request reads even when no selection exists yet.
    PgReviewSubmissionStore::read_review_request_in_transaction(tx, id)
        .await?
        .ok_or(SubmissionStoreError::Invalid)?;
    let Some(row) = tx.query_opt("SELECT selection_bytes,checksum FROM reborn_component_validator_selections WHERE submission_id=$1", &[&id]).await.map_err(database)? else { return Ok(None) };
    let bytes: String = row.get(0);
    if hex(Sha256::digest(bytes.as_bytes()).into()) != row.get::<_, String>(1) {
        return Err(SubmissionStoreError::Invalid);
    }
    let retained = verify(tx, &bytes, limits).await?;
    if retained.submission_id != id {
        return Err(SubmissionStoreError::Invalid);
    }
    Ok(Some(retained))
}
