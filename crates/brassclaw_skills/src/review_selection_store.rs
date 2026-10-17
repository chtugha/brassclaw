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
    bytes: String,
    validator_bytes: String,
    selection: serde_json::Value,
    snapshot: std::sync::Arc<crate::component_revision::RetainedComponentSnapshot>,
}
impl RetainedValidatorSelection {
    pub fn bytes(&self) -> &str {
        &self.bytes
    }
    pub fn validator_bytes(&self) -> &str {
        &self.validator_bytes
    }
    pub fn selection(&self) -> &serde_json::Value {
        &self.selection
    }
    pub fn snapshot(&self) -> std::sync::Arc<crate::component_revision::RetainedComponentSnapshot> {
        self.snapshot.clone()
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
}

async fn verify(
    tx: &Transaction<'_>,
    bytes: &str,
    limits: ContractLimits,
) -> Result<RetainedValidatorSelection, SubmissionStoreError> {
    let value = strict_json(bytes, limits).map_err(|_| SubmissionStoreError::Invalid)?;
    let value: Selection =
        serde_json::from_value(value).map_err(|_| SubmissionStoreError::Invalid)?;
    if value.format != "component-validator-selection/1" || value.scope != "preparation-only" {
        return Err(SubmissionStoreError::Invalid);
    }
    let subject = PgReviewSubmissionStore::read_in_transaction(tx, value.submission_id)
        .await?
        .ok_or(SubmissionStoreError::Invalid)?;
    if !matches!(
        subject.candidate.class_code,
        0 | 1 | 2 | 3 | 13 | 21 | 22 | 23
    ) {
        return Err(SubmissionStoreError::Invalid);
    }
    if subject.checksum != value.subject_checksum
        || subject.candidate != value.candidate.parsed()?
        || value.validator_selection_checksum
            != hex(Sha256::digest(value.validator_selection_bytes.as_bytes()).into())
    {
        return Err(SubmissionStoreError::Invalid);
    }
    let selection = strict_json(&value.validator_selection_bytes, limits)
        .map_err(|_| SubmissionStoreError::Invalid)?;
    if selection["format"] != "monty-retained-recipe-selection/1" {
        return Err(SubmissionStoreError::Invalid);
    }
    let fields = [
        "format",
        "recipe",
        "variant",
        "workflow_class",
        "input_layout",
        "invocation_layout",
        "step_order",
        "components",
    ];
    let object = selection.as_object().ok_or(SubmissionStoreError::Invalid)?;
    if fields.iter().any(|name| !object.contains_key(*name))
        || object
            .keys()
            .any(|name| !fields.contains(&name.as_str()) && name != "association_approvals")
        || !matches!(
            selection["workflow_class"].as_str(),
            Some("deterministic" | "requires_model")
        )
        || selection["step_order"].as_array().is_none_or(Vec::is_empty)
        || !selection["input_layout"].is_object()
    {
        return Err(SubmissionStoreError::Invalid);
    }
    let root: Reference = serde_json::from_value(selection["recipe"].clone())
        .map_err(|_| SubmissionStoreError::Invalid)?;
    let root = root.parsed()?;
    let refs: Vec<Reference> = serde_json::from_value(selection["components"].clone())
        .map_err(|_| SubmissionStoreError::Invalid)?;
    let refs = refs
        .iter()
        .map(Reference::parsed)
        .collect::<Result<Vec<_>, _>>()?;
    let graph =
        PgComponentRevisionStore::read_exact_in_transaction(tx, &[root.uuid], &refs).await?;
    if root.class_code != 21 || graph.revisions()[&root.uuid].reference() != root {
        return Err(SubmissionStoreError::Invalid);
    }
    let doc = graph.revisions()[&root.uuid].draft().document();
    let key = selection["variant"]["variant_key"]
        .as_str()
        .ok_or(SubmissionStoreError::Invalid)?;
    let variant = doc["variants"]
        .as_array()
        .and_then(|variants| variants.iter().find(|v| v["variant_key"] == key))
        .ok_or(SubmissionStoreError::Invalid)?;
    if doc["validates_class_code"].as_i64() != Some(i64::from(subject.candidate.class_code))
        || doc["consumer_tags"] != serde_json::json!(["05:validator"])
        || variant["intent_examples"] != serde_json::json!([])
        || selection["variant"]["intent_examples"] != serde_json::json!([])
        || variant["step_link"] != selection["variant"]["step_link"]
        || doc["input_layouts"][key] != selection["input_layout"]
        || doc.get("invocations").unwrap_or(&serde_json::Value::Null)
            != &selection["invocation_layout"]
    {
        return Err(SubmissionStoreError::Invalid);
    }
    Ok(RetainedValidatorSelection {
        submission_id: value.submission_id,
        bytes: bytes.to_owned(),
        validator_bytes: value.validator_selection_bytes,
        selection,
        snapshot: std::sync::Arc::new(graph),
    })
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
