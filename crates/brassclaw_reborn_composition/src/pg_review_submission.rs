//! Operator authoring boundary. Only immutable, unreviewed subjects are written;
//! explicit queue capture may create its pending legacy projection atomically.
//! No evidence record or active catalogue is changed here.
use std::sync::Arc;

use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    ComponentFunctionReviewPartner, ComponentFunctionReviewPartnerReceipt,
    ComponentReviewSubmissionReceipt, ComponentReviewSubmissionView, ComponentRevisionSelection,
    RecipeStoreError, SubmitComponentReviewRequest,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::ComponentRevisionDraft,
    review_submission_store::{
        FunctionReviewPartnerDraft, PgReviewSubmissionStore, RetainedReviewSubmission,
        ReviewSubmissionDraft, SubmissionStoreError,
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
pub(crate) fn reference(
    value: &ComponentRevisionSelection,
) -> Result<ComponentRevisionRef, RecipeStoreError> {
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
pub(crate) fn selection(value: ComponentRevisionRef) -> ComponentRevisionSelection {
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
        queue_ids: vec![],
        queue_binding_status: brassclaw_product_workflow::ReviewQueueBindingStatus::Standalone,
        submission_id: value.id.to_string(),
        subject_checksum: value.checksum.clone(),
        candidate: selection(value.candidate),
        base: value.base.map(selection),
        dependencies: value.dependencies.iter().copied().map(selection).collect(),
        function_partner: value.function_partner.map(|partner| {
            ComponentFunctionReviewPartnerReceipt {
                reference: selection(partner.reference),
                changed: partner.changed,
                base: partner.base.map(selection),
            }
        }),
        review_status: "unreviewed".into(),
        catalogue_activated: false,
    }
}
async fn original_receipt(
    pool: &Arc<PgPool>,
    retained: &RetainedReviewSubmission,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    let mut result = receipt(retained);
    let client = pool.get().await.map_err(storage_io)?;
    if let Some(row) = client
        .query_opt(
            "SELECT queue_ids FROM reborn_component_review_queue_requests WHERE submission_id=$1",
            &[&retained.id],
        )
        .await
        .map_err(storage_io)?
    {
        result.queue_ids = row
            .get::<_, Vec<Uuid>>(0)
            .into_iter()
            .map(|id| id.to_string())
            .collect();
    }
    if !result.queue_ids.is_empty() {
        result.queue_binding_status = brassclaw_product_workflow::ReviewQueueBindingStatus::Bound;
    }
    Ok(result)
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
fn storage_io(_: impl std::fmt::Display) -> RecipeStoreError {
    RecipeStoreError::Unavailable(
        "original review storage unavailable; retain original request for readback".into(),
    )
}
pub(crate) async fn submit(
    pool: &Arc<PgPool>,
    actor: &str,
    tenant: &str,
    agent: &str,
    request: SubmitComponentReviewRequest,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    if request.dependencies.len() > 4095 {
        return Err(RecipeStoreError::Invalid(
            "dependency selection exceeds capacity".into(),
        ));
    }
    let queue_ids = request
        .queue_ids
        .iter()
        .map(|id| identifier(id))
        .collect::<Result<Vec<_>, _>>()?;
    let id = identifier(&request.submission_id)?;
    let candidate = ComponentRevisionDraft::from_json(&request.candidate_bytes)
        .map_err(|_| RecipeStoreError::Invalid("invalid immutable candidate revision".into()))?;
    let base = request.base.as_ref().map(reference).transpose()?;
    let dependencies = request
        .dependencies
        .iter()
        .map(reference)
        .collect::<Result<Vec<_>, _>>()?;
    let draft = match (request.expected_previous_version, request.function_partner) {
        (Some(head), Some(partner)) => {
            let partner = match partner {
                ComponentFunctionReviewPartner::Changed {
                    candidate_bytes,
                    base,
                    expected_previous_version,
                } => FunctionReviewPartnerDraft::Changed {
                    candidate: Box::new(
                        ComponentRevisionDraft::from_json(&candidate_bytes).map_err(|_| {
                            RecipeStoreError::Invalid("invalid immutable partner revision".into())
                        })?,
                    ),
                    base: base.as_ref().map(reference).transpose()?,
                    expected_previous_version,
                },
                ComponentFunctionReviewPartner::Context { reference: value } => {
                    FunctionReviewPartnerDraft::Context {
                        reference: reference(&value)?,
                    }
                }
            };
            ReviewSubmissionDraft::new_function(
                id,
                actor,
                candidate,
                base,
                dependencies,
                head,
                partner,
            )
        }
        (None, Some(_)) => {
            return Err(RecipeStoreError::Invalid(
                "function review requires each changed allocation head".into(),
            ));
        }
        (Some(head), None) => {
            ReviewSubmissionDraft::new_after_head(id, actor, candidate, base, dependencies, head)
        }
        (None, None) => ReviewSubmissionDraft::new(id, actor, candidate, base, dependencies),
    }
    .map_err(storage)?;
    if let Some(project) = request.create_queue_project {
        if project.is_empty()
            || !queue_ids.is_empty()
            || draft.changed_roots().map_err(storage)?.len() != 1
        {
            return Err(RecipeStoreError::Invalid(
                "one explicit new queue projection required".into(),
            ));
        }
        let retained = crate::queued_review_submission::create(
            pool,
            brassclaw_engine::memory::retrieval_source::ComponentScope {
                tenant_id: tenant.into(),
                user_id: actor.into(),
                agent_id: agent.into(),
                project_id: project,
            },
            &draft,
        )
        .await?;
        return original_receipt(pool, &retained).await;
    }
    let retained = {
        let mut client = pool.get().await.map_err(storage_io)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(storage_io)?;
        let mut targets = Vec::new();
        // The explicit selector is retained; no component/latest lookup chooses
        // the subject. Locks and complete payload checks happen in the helper.
        for queue_id in &queue_ids {
            let row=tx.query_opt("SELECT tenant_id,user_id,agent_id,project_id,component_id FROM reborn_validation_queue WHERE id=$1",&[queue_id]).await.map_err(storage_io)?;
            if let Some(row) = row {
                targets.push((
                    brassclaw_engine::memory::retrieval_source::ComponentScope {
                        tenant_id: row.get(0),
                        user_id: row.get(1),
                        agent_id: row.get(2),
                        project_id: row.get(3),
                    },
                    row.get(4),
                    *queue_id,
                ));
            } else {
                // Historical original readback is valid after Q2 queue removal.
                let old = brassclaw_skills::review_submission_store::queue::read_in_transaction(
                    &tx, *queue_id,
                )
                .await
                .map_err(storage)?
                .ok_or_else(|| RecipeStoreError::NotFound("queue capture not found".into()))?;
                if old.submission_id() != id {
                    return Err(RecipeStoreError::Conflict(
                        "queue belongs to another original review".into(),
                    ));
                }
            }
        }
        let retained =
            crate::validation_queue::ValidationQueueStore::submit_original_in_transaction(
                &tx,
                &draft,
                &targets,
                &queue_ids,
                if queue_ids.is_empty() {
                    "standalone"
                } else {
                    "existing_queue"
                },
            )
            .await
            .map_err(crate::validation_queue::original_review_error)?;
        tx.commit().await.map_err(storage_io)?;
        retained
    };
    original_receipt(pool, &retained).await
}
pub(crate) async fn head(
    pool: &Arc<PgPool>,
    component_id: &str,
) -> Result<brassclaw_product_workflow::ComponentRevisionHeadView, RecipeStoreError> {
    let id = identifier(component_id)?;
    let value = brassclaw_skills::revision_store::PgComponentRevisionStore::new(pool.clone())
        .read_allocation_head(id)
        .await
        .map_err(|error| storage(SubmissionStoreError::Revisions(error)))?;
    Ok(brassclaw_product_workflow::ComponentRevisionHeadView {
        component_id: id.to_string(),
        class_code: value.map(|(class, _)| class),
        last_allocated_version: value.map_or(0, |(_, head)| head),
    })
}

pub(crate) async fn get(
    pool: &Arc<PgPool>,
    submission_id: &str,
) -> Result<ComponentReviewSubmissionView, RecipeStoreError> {
    let id = identifier(submission_id)?;
    let mut client = pool
        .get()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review connection failed".into()))?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review snapshot unavailable".into()))?;
    let retained = PgReviewSubmissionStore::read_in_transaction(&tx, id)
        .await
        .map_err(storage)?
        .ok_or_else(|| RecipeStoreError::NotFound("review submission not found".into()))?;
    let saved =
        brassclaw_skills::review_submission_store::selection::read_preparation_in_transaction(
            &tx,
            id,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .await
        .map_err(storage)?;
    if let Some(saved) = &saved {
        brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage::restore_preparation(saved)
            .map_err(|reason| RecipeStoreError::Invalid(reason.into()))?;
    }
    let review_selection_bytes = saved.map(|saved| saved.bytes().to_owned());
    let mut retained_receipt = receipt(&retained);
    if let Some(row) = tx
        .query_opt(
            "SELECT queue_ids FROM reborn_component_review_queue_requests WHERE submission_id=$1",
            &[&retained.id],
        )
        .await
        .map_err(storage_io)?
    {
        retained_receipt.queue_ids = row
            .get::<_, Vec<Uuid>>(0)
            .into_iter()
            .map(|id| id.to_string())
            .collect();
    }

    if !retained_receipt.queue_ids.is_empty() {
        retained_receipt.queue_binding_status =
            brassclaw_product_workflow::ReviewQueueBindingStatus::Bound;
    }
    #[cfg(feature = "skills-db")]
    let work = Some(
        crate::review_work_reconciliation::inspect(&tx, id)
            .await
            .map_err(|_| {
                RecipeStoreError::Unavailable("original review work requires reconciliation".into())
            })?,
    );
    #[cfg(not(feature = "skills-db"))]
    let work = None;
    tx.commit()
        .await
        .map_err(|_| RecipeStoreError::Unavailable("review snapshot commit unavailable".into()))?;
    Ok(ComponentReviewSubmissionView {
        receipt: retained_receipt,
        subject_bytes: retained.subject_bytes,
        component_bytes: retained
            .snapshot
            .revisions()
            .values()
            .map(|revision| revision.draft().exact_bytes().to_owned())
            .collect(),
        review_selection_bytes,
        work,
    })
}

#[cfg(test)]
#[path = "pg_review_submission_tests.rs"]
mod tests;

#[cfg(all(test, feature = "skills-db"))]
#[path = "pg_review_model_selection_tests.rs"]
mod model_selection_tests;

#[cfg(test)]
#[path = "pg_function_review_submission_tests.rs"]
mod function_tests;

#[cfg(test)]
#[path = "original_queue_binding_tests.rs"]
mod queue_tests;
