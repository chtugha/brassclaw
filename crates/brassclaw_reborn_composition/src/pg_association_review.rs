//! Human-only exact usage review, reached through RecipeStore/RebornServices.
//! Host composition mounts these routes only for the instance operator.
//! Ordinary authoring/model/Recipe ports cannot write successful Q2 records.

use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    ApproveAssociationRequest, AssociationApprovalResponse, AssociationReviewSelection,
    AssociationReviewView, RecipeStoreError,
};
use brassclaw_skills::association_review_store::{
    AssociationReviewStoreError, HumanAssociationDecision, prepare_human_association_review,
    record_human_association_approval,
};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

fn identifier(text: &str) -> Result<Uuid, RecipeStoreError> {
    Uuid::parse_str(text)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == text)
        .ok_or_else(|| RecipeStoreError::Invalid("canonical non-nil UUID required".into()))
}

fn selection(value: &AssociationReviewSelection) -> Result<(Uuid, Vec<Uuid>), RecipeStoreError> {
    if value.behavioral_refs.is_empty() || value.behavioral_refs.len() > 254 {
        return Err(RecipeStoreError::Invalid(
            "behavioral evidence selection exceeds capacity".into(),
        ));
    }
    Ok((
        identifier(&value.q1_ref)?,
        value
            .behavioral_refs
            .iter()
            .map(|id| identifier(id))
            .collect::<Result<_, _>>()?,
    ))
}

fn storage(error: AssociationReviewStoreError) -> RecipeStoreError {
    match error {
        AssociationReviewStoreError::Database(_) => {
            RecipeStoreError::Unavailable(error.to_string())
        }
        AssociationReviewStoreError::Revisions(
            brassclaw_skills::revision_store::RevisionStoreError::Database(_),
        ) => RecipeStoreError::Unavailable(error.to_string()),
        _ => RecipeStoreError::Invalid(error.to_string()),
    }
}

pub(crate) async fn prepare(
    pool: &PgPool,
    skill_id: &str,
    value: AssociationReviewSelection,
) -> Result<AssociationReviewView, RecipeStoreError> {
    let skill = identifier(skill_id)?;
    let (q1, behaviors) = selection(&value)?;
    let mut client = pool
        .get()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review connection failed".into()))?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review transaction failed".into()))?;
    let review = prepare_human_association_review(&tx, skill, q1, &behaviors)
        .await
        .map_err(storage)?;
    let view = AssociationReviewView {
        review_checksum: review.checksum().into(),
        review: review.view().clone(),
    };
    tx.commit()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review read completion failed".into()))?;
    Ok(view)
}

pub(crate) async fn approve(
    pool: &PgPool,
    actor: &str,
    skill_id: &str,
    request: ApproveAssociationRequest,
) -> Result<AssociationApprovalResponse, RecipeStoreError> {
    let skill = identifier(skill_id)?;
    let approval = identifier(&request.approval_id)?;
    let q2 = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("brassclaw:human-association-q2:{approval}").as_bytes(),
    );
    let (q1, behaviors) = selection(&request.selection)?;
    if request.semantic_review.trim().is_empty()
        || request.semantic_review.len() > 16_384
        || actor.trim().is_empty()
        || actor.len() > 2048
    {
        return Err(RecipeStoreError::Invalid(
            "explicit human semantic review required".into(),
        ));
    }
    let mut client = pool
        .get()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review connection failed".into()))?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review transaction failed".into()))?;
    let review = prepare_human_association_review(&tx, skill, q1, &behaviors)
        .await
        .map_err(storage)?;
    record_human_association_approval(
        &tx,
        &review,
        HumanAssociationDecision {
            approval_id: approval,
            q2_id: q2,
            actor,
            semantic_review: &request.semantic_review,
            reviewed_checksum: &request.review_checksum,
        },
    )
    .await
    .map_err(storage)?;
    // A serialization conflict or ambiguous commit must remain a retriable
    // failure. The client retains the same request/ID; no new approval is made.
    tx.commit()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review commit requires exact retry".into()))?;
    Ok(AssociationApprovalResponse {
        approval_id: approval.to_string(),
        q2_ref: q2.to_string(),
        review_checksum: review.checksum().into(),
        catalogue_activated: false,
    })
}

#[cfg(test)]
#[path = "pg_association_review_tests.rs"]
mod tests;
