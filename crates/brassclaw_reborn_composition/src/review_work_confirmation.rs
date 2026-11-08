//! The review owner acknowledges its original delivered structural observations.
//! No full class-Q1 qualification, human Q2 or activation is produced here.
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{PgReviewSubmissionStore, queue, selection},
};
use brassclaw_turns::{private_work::PrivateExecutionContext, run_profile::AgentLoopDriverError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

fn invalid() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "review_work_original_conflict".into(),
    }
}
fn database(_: tokio_postgres::Error) -> AgentLoopDriverError {
    AgentLoopDriverError::Unavailable {
        reason: "review work original readback unavailable".into(),
    }
}

struct Prepared {
    delivery: crate::pg_monty_admission::DeliveredPrivateCompletion,
    subject: brassclaw_skills::review_submission_store::RetainedValidatorSubject,
    queue_ids: Vec<Uuid>,
    actor: String,
    record: Value,
    bytes: String,
    checksum: [u8; 32],
    succeeded: bool,
    errors: Vec<String>,
}
async fn prepare(
    tx: &Transaction<'_>,
    id: Uuid,
    context: &PrivateExecutionContext,
) -> Result<Option<Prepared>, AgentLoopDriverError> {
    let Some(delivery) =
        crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(tx, id, context).await?
    else {
        return Ok(None);
    };
    let subject =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(tx, id, REVISION_LIMITS)
            .await
            .map_err(|_| invalid())?
            .ok_or_else(invalid)?;
    let selected = selection::read_preparation_in_transaction(tx, id, REVISION_LIMITS)
        .await
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let verdict = crate::pg_validator_q1_evidence::read_delivered_validator_q1(
        tx, &subject, &selected, &delivery,
    )
    .await
    .map_err(|_| invalid())?;
    let original = tx
        .query_one(
            "SELECT q.queue_ids,s.actor FROM reborn_component_review_queue_requests q
        JOIN reborn_component_review_submissions s USING(submission_id) WHERE q.submission_id=$1",
            &[&id],
        )
        .await
        .map_err(database)?;
    let actor: String = original.get(1);
    if context.actor.as_ref().map(|actor| actor.user_id.as_str()) != Some(actor.as_str()) {
        return Err(invalid());
    }
    let queue_ids: Vec<Uuid> = original.get(0);
    let mut bindings = Vec::new();
    for queue_id in &queue_ids {
        let binding = queue::read_in_transaction(tx, *queue_id)
            .await
            .map_err(|_| invalid())?
            .ok_or_else(invalid)?;
        binding.require_subject(&subject).map_err(|_| invalid())?;
        bindings.push(json!({"queue_id":queue_id,"checksum":binding.checksum()}));
    }
    let record = json!({"format":"component-review-work-confirmation/1","scope":"structural-observations-only",
        "submission_id":id,"subject_checksum":subject.subject_checksum(),"work_id":delivery.original().request().work_id(),
        "run_id":delivery.original().request().run_id().as_uuid(),"delivery_checksum":delivery.checksum(),
        "validator_selection_checksum":hex::encode(Sha256::digest(selected.validator_bytes().as_bytes())),
        "evidence_id":verdict.evidence_id,"evidence_checksum":verdict.evidence_checksum,
        "checks_succeeded":verdict.succeeded,"errors":verdict.errors,"queue_bindings":bindings,
        "q1_qualified":false,"human_q2":false,"activation":false});
    let bytes = serde_json::to_string(&record).map_err(|_| invalid())?;
    let checksum: [u8; 32] = Sha256::digest(bytes.as_bytes()).into();
    Ok(Some(Prepared {
        delivery,
        subject,
        queue_ids,
        actor,
        record,
        bytes,
        checksum,
        succeeded: verdict.succeeded,
        errors: verdict.errors,
    }))
}

async fn read_prepared(
    tx: &Transaction<'_>,
    id: Uuid,
    bytes: &str,
    checksum: [u8; 32],
) -> Result<bool, AgentLoopDriverError> {
    let Some(row)=tx.query_opt("SELECT confirmation_bytes,checksum FROM reborn_component_review_work_confirmations WHERE submission_id=$1",&[&id]).await.map_err(database)? else {return Ok(false);};
    if row.get::<_, &str>(0) != bytes || row.get::<_, Vec<u8>>(1).as_slice() != checksum {
        return Err(invalid());
    }
    Ok(true)
}
/// Strict readback after original mutable queues have legitimately disappeared.
pub(crate) async fn read(
    tx: &Transaction<'_>,
    id: Uuid,
    context: &PrivateExecutionContext,
) -> Result<Option<Value>, AgentLoopDriverError> {
    let Some(prepared) = prepare(tx, id, context).await? else {
        return Ok(None);
    };
    if read_prepared(tx, id, &prepared.bytes, prepared.checksum).await? {
        Ok(Some(prepared.record))
    } else {
        Ok(None)
    }
}
/// The caller commits this original acknowledgement and any permitted negative
/// diagnostics together. Successful structural observations cannot write state 2.
pub(crate) async fn confirm(
    tx: &Transaction<'_>,
    id: Uuid,
    context: &PrivateExecutionContext,
) -> Result<Option<Value>, AgentLoopDriverError> {
    let Some(Prepared {
        delivery,
        subject,
        queue_ids,
        actor,
        record,
        bytes,
        checksum,
        succeeded,
        errors,
    }) = prepare(tx, id, context).await?
    else {
        return Ok(None);
    };
    // Serializes absent receipts without introducing a worker lease or grant.
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&format!("review-work-confirmation:{id}")],
    )
    .await
    .map_err(database)?;
    if read_prepared(tx, id, &bytes, checksum).await? {
        return Ok(Some(record));
    }
    for queue_id in queue_ids {
        let row=tx.query_one("SELECT tenant_id,user_id,agent_id,project_id,component_id FROM reborn_validation_queue WHERE id=$1",&[&queue_id]).await.map_err(database)?;
        let scope = ComponentScope {
            tenant_id: row.get(0),
            user_id: row.get(1),
            agent_id: row.get(2),
            project_id: row.get(3),
        };
        // Component storage scope and private execution context are distinct.
        // The exact original binding, not a new role/scope grant, supplies the
        // candidate scope. The subject's original author must still agree.
        if scope.user_id != actor {
            return Err(invalid());
        }
        let candidate =
            crate::validation_queue::ValidationQueueStore::capture_candidate_in_transaction(
                tx,
                &scope,
                row.get(4),
            )
            .await
            .map_err(|_| invalid())?;
        let bound = candidate.original().map_err(|_| invalid())?;
        if bound.queue_id() != queue_id {
            return Err(invalid());
        }
        bound.require_subject(&subject).map_err(|_| invalid())?;
        if !succeeded {
            crate::validation_queue::ValidationQueueStore::record_structural_failure_in_transaction(tx,&scope,&candidate,&errors).await.map_err(|_|invalid())?;
        }
    }
    tx.execute("INSERT INTO reborn_component_review_work_confirmations(submission_id,work_id,delivery_checksum,confirmation_bytes,checksum)
        VALUES($1,$2,$3,$4,$5)",&[&id,&delivery.original().request().work_id(),&&delivery.checksum()[..],&bytes,&&checksum[..]])
        .await.map_err(database)?;
    Ok(Some(record))
}
