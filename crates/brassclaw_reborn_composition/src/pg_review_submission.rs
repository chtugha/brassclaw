//! Operator authoring boundary. Only immutable, unreviewed subjects are written;
//! no legacy live row, evidence record or active catalogue is changed here.
use std::sync::Arc;

use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    ComponentReviewSubmissionReceipt, ComponentReviewSubmissionView, ComponentRevisionSelection,
    RecipeStoreError, SubmitComponentReviewRequest,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::ComponentRevisionDraft,
    review_submission_store::{
        PgReviewSubmissionStore, RetainedReviewSubmission, ReviewSubmissionDraft,
        SubmissionStoreError,
    },
    revision_store::RevisionStoreError,
};
use uuid::Uuid;

fn identifier(value: &str) -> Result<Uuid, RecipeStoreError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| RecipeStoreError::Invalid("canonical non-nil UUID required".into()))
}
fn reference(value: &ComponentRevisionSelection) -> Result<ComponentRevisionRef, RecipeStoreError> {
    if value.version == 0
        || value.version > i64::MAX as u64
        || !matches!(value.class_code, 0..=10 | 12..=23 | 50)
        || value.checksum.len() != 64
        || !value
            .checksum
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(RecipeStoreError::Invalid(
            "exact revision reference required".into(),
        ));
    }
    let mut checksum = [0; 32];
    for (slot, pair) in checksum
        .iter_mut()
        .zip(value.checksum.as_bytes().as_chunks::<2>().0.iter())
    {
        let digit = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
        *slot = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(ComponentRevisionRef {
        uuid: identifier(&value.uuid)?,
        class_code: value.class_code,
        version: value.version,
        checksum,
    })
}
fn selection(value: ComponentRevisionRef) -> ComponentRevisionSelection {
    ComponentRevisionSelection {
        uuid: value.uuid.to_string(),
        class_code: value.class_code,
        version: value.version,
        checksum: value
            .checksum
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}
fn receipt(value: &RetainedReviewSubmission) -> ComponentReviewSubmissionReceipt {
    ComponentReviewSubmissionReceipt {
        submission_id: value.id.to_string(),
        subject_checksum: value.checksum.clone(),
        candidate: selection(value.candidate),
        base: value.base.map(selection),
        dependencies: value.dependencies.iter().copied().map(selection).collect(),
        review_status: "unreviewed".into(),
        catalogue_activated: false,
    }
}
fn storage(value: SubmissionStoreError) -> RecipeStoreError {
    match value {
        SubmissionStoreError::Conflict
        | SubmissionStoreError::Revisions(RevisionStoreError::Conflict) => {
            RecipeStoreError::Conflict(value.to_string())
        }
        SubmissionStoreError::Database(_)
        | SubmissionStoreError::Revisions(RevisionStoreError::Database(_)) => {
            RecipeStoreError::Unavailable(value.to_string())
        }
        _ => RecipeStoreError::Invalid(value.to_string()),
    }
}
pub(crate) async fn submit(
    pool: &Arc<PgPool>,
    actor: &str,
    request: SubmitComponentReviewRequest,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    if request.dependencies.len() > 4095 {
        return Err(RecipeStoreError::Invalid(
            "dependency selection exceeds capacity".into(),
        ));
    }
    let id = identifier(&request.submission_id)?;
    let candidate = ComponentRevisionDraft::from_json(&request.candidate_bytes)
        .map_err(|_| RecipeStoreError::Invalid("invalid immutable candidate revision".into()))?;
    let base = request.base.as_ref().map(reference).transpose()?;
    let dependencies = request
        .dependencies
        .iter()
        .map(reference)
        .collect::<Result<Vec<_>, _>>()?;
    let draft =
        ReviewSubmissionDraft::new(id, actor, candidate, base, dependencies).map_err(storage)?;
    let retained = PgReviewSubmissionStore::new(pool.clone())
        .submit(&draft)
        .await
        .map_err(storage)?;
    Ok(receipt(&retained))
}
pub(crate) async fn get(
    pool: &Arc<PgPool>,
    submission_id: &str,
) -> Result<ComponentReviewSubmissionView, RecipeStoreError> {
    let id = identifier(submission_id)?;
    let retained = PgReviewSubmissionStore::new(pool.clone())
        .read(id)
        .await
        .map_err(storage)?
        .ok_or_else(|| RecipeStoreError::NotFound("review submission not found".into()))?;
    Ok(ComponentReviewSubmissionView {
        receipt: receipt(&retained),
        subject_bytes: retained.subject_bytes,
        component_bytes: retained
            .snapshot
            .revisions()
            .values()
            .map(|revision| revision.draft().exact_bytes().to_owned())
            .collect(),
    })
}

#[cfg(test)]
#[path = "pg_review_submission_tests.rs"]
mod tests;
