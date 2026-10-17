//! Native settlement/accounting recovery only. Never re-admit a review, invoke
//! a provider, replay a Tool, finish a Recipe or activate a candidate.
use super::{backend::checksum, ports::ReviewPorts};
use brassclaw_monty_host::service::{TaskOutcome, TaskReceipt};
use brassclaw_pg::PgPool;
use brassclaw_skills::{completed_turn_analysis, review_submission_store::PgReviewSubmissionStore};
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) fn settlement(receipt: &TaskReceipt) -> String {
    json!({"format":"completed-turn-review-service-settlement/1",
        "root_vm_id":receipt.root.vm_id(),"accounting":receipt.accounting.as_ref().map(|a|
            json!({"effective_revision":a.effective_revision,"compute_time":a.compute_time,"failure":a.failure})),
        "outcome":match &receipt.outcome {
            TaskOutcome::InternalCompleted {receipt_ref}=>json!({"status":"internal_completed","receipt_ref":receipt_ref}),
            TaskOutcome::Failed {reason_kind}=>json!({"status":"failed","reason_kind":reason_kind}),
            _=>json!({"status":"invalid_chat_completion"}),
        },
        "internal_completed":matches!(receipt.outcome,TaskOutcome::InternalCompleted {..}),
        "withheld_root_answers":receipt.withheld.len()}).to_string()
}

pub(super) fn local_settled(receipt: &TaskReceipt) -> bool {
    matches!(
        receipt.outcome,
        TaskOutcome::Failed { .. } | TaskOutcome::InternalCompleted { .. }
    ) && receipt.withheld.is_empty()
        && receipt
            .accounting
            .as_ref()
            .is_some_and(|a| a.compute_time.is_some() && a.failure.is_none())
}

/// Returns true only after an immutable reconciliation record is read back.
/// An absent native owner/receipt cannot establish settlement after restart.
pub(super) async fn reconcile(
    pool: &Arc<PgPool>,
    ports: &ReviewPorts,
    receipt: &TaskReceipt,
) -> Result<bool, &'static str> {
    if !local_settled(receipt) {
        return Ok(false);
    }
    let backend = &ports.backend;
    let attempt = backend.attempt;
    let client = pool
        .get()
        .await
        .map_err(|_| "reconciliation_storage_unavailable")?;
    let work = client.query_one("SELECT owner_id,phase,model_dispatch_count,response_bytes,selection_bytes,prefix_bytes,model_identity
        FROM brassclaw_monty_review_work WHERE attempt_id=$1", &[&attempt]).await.map_err(|_| "reconciliation_storage_unavailable")?;
    if work.get::<_, uuid::Uuid>(0) != backend.owner
        || !matches!(
            work.get::<_, String>(1).as_str(),
            "acknowledged" | "incomplete" | "failed" | "uncertain"
        )
        || work.get::<_, String>(4) != backend.selection
        || work.get::<_, String>(5) != backend.model.prefix
        || work.get::<_, String>(6) != backend.model.name
    {
        return Ok(false);
    }
    if let TaskOutcome::InternalCompleted { receipt_ref } = &receipt.outcome
        && (receipt_ref != &format!("review:{attempt}")
            || work.get::<_, String>(1) != "acknowledged")
    {
        return Ok(false);
    }
    let exact_settlement = settlement(receipt);
    client
        .execute(
            "INSERT INTO brassclaw_monty_review_settlements(attempt_id,settlement_bytes)
        VALUES($1,$2) ON CONFLICT DO NOTHING",
            &[&attempt, &exact_settlement],
        )
        .await
        .map_err(|_| "reconciliation_storage_unavailable")?;
    let recorded: String = client
        .query_one(
            "SELECT settlement_bytes FROM brassclaw_monty_review_settlements WHERE attempt_id=$1",
            &[&attempt],
        )
        .await
        .map_err(|_| "reconciliation_storage_unavailable")?
        .get(0);
    if recorded != exact_settlement {
        return Ok(false);
    }
    let answers = ports.retained_answers().await;
    let settled_operations = ports.settled_operations().await;
    let operations = client.query("SELECT operation,input_bytes,input_checksum,output_bytes,output_checksum FROM brassclaw_monty_review_operations
        WHERE attempt_id=$1 ORDER BY operation", &[&attempt]).await.map_err(|_| "reconciliation_storage_unavailable")?;
    // Every intent must have an actual observed native answer. A synthetic
    // terminal error or an operator note cannot fill a lost host observation.
    for row in &operations {
        let operation: String = row.get(0);
        if !settled_operations.contains(&operation) {
            return Ok(false);
        }
        let input: String = row.get(1);
        let Some(answer) = answers.get(&operation) else {
            return Ok(false);
        };
        if checksum(&input) != row.get::<_, String>(2) {
            return Ok(false);
        }
        let original: Option<String> = row.get(3);
        if original.as_ref().is_some_and(|value| value != answer)
            || original
                .as_ref()
                .is_some_and(|value| Some(checksum(value)) != row.get::<_, Option<String>>(4))
        {
            return Ok(false);
        }
        if original.is_none() {
            client.execute("UPDATE brassclaw_monty_review_operations SET output_bytes=$3,output_checksum=$4,answered_at=clock_timestamp()
                WHERE attempt_id=$1 AND operation=$2 AND output_bytes IS NULL", &[&attempt,&operation,answer,&checksum(answer)])
                .await.map_err(|_| "reconciliation_storage_unavailable")?;
            let readback: Option<String> = client.query_one("SELECT output_bytes FROM brassclaw_monty_review_operations WHERE attempt_id=$1 AND operation=$2",
                &[&attempt,&operation]).await.map_err(|_| "reconciliation_storage_unavailable")?.get(0);
            if readback.as_ref() != Some(answer) {
                return Ok(false);
            }
        }
    }
    if answers.len() != operations.len() {
        return Ok(false);
    }
    let observed = backend.observed_model.lock().await.clone();
    if work
        .get::<_, Option<String>>(3)
        .is_some_and(|value| Some(value) != observed)
    {
        return Ok(false);
    }
    {
        let model = backend.model_work.lock().await;
        if (work.get::<_, i32>(2) == 1 && model.request.is_none())
            || (observed.is_some() && model.response.is_none())
        {
            return Ok(false);
        }
    }
    // Resolve every possible batch subject by immutable ID and exact content.
    // Missing subjects after native quiescence are absent, not invitations to
    // submit them. Partial success is preserved without completing the Recipe.
    let mut submissions = Vec::new();
    if answers.contains_key("submit_candidates") {
        let model = backend.model_work.lock().await;
        let Some(response) = &model.response else {
            return Ok(false);
        };
        let brassclaw_turns::run_profile::ParentLoopOutput::AssistantReply(reply) =
            &response.output
        else {
            return Ok(false);
        };
        let decoded = completed_turn_analysis::decode(attempt, &reply.content)
            .map_err(|_| "reconciliation_subject_invalid")?;
        let store = PgReviewSubmissionStore::new(pool.clone());
        for mut candidate in decoded.candidates {
            candidate.dependencies.sort();
            let actual = store
                .read(candidate.submission_id)
                .await
                .map_err(|_| "reconciliation_subject_unavailable")?;
            if let Some(subject) = actual {
                let value: Value = serde_json::from_str(&subject.subject_bytes)
                    .map_err(|_| "reconciliation_subject_invalid")?;
                if subject.actor != "completed-turn-sempai"
                    || subject.base != candidate.base
                    || subject.dependencies != candidate.dependencies
                    || value["candidate_bytes"].as_str() != Some(candidate.candidate.exact_bytes())
                {
                    return Ok(false);
                }
                submissions.push(json!({"submission_id":subject.id,"status":"retained","subject_checksum":subject.checksum}));
            } else {
                submissions.push(json!({"submission_id":candidate.submission_id,"status":"absent_after_native_settlement"}));
            }
        }
    }
    if !backend.reconcile_accounting().await {
        return Ok(false);
    }
    let evidence = json!({"format":"completed-turn-review-reconciliation/1","attempt_id":attempt,
        "owner_id":backend.owner,"settlement_bytes":exact_settlement,"native_answers":answers,"primitive_accounting_settled":settled_operations,
        "observed_model_bytes":observed,"model_accounting_settled":true,
        "submissions":submissions,"work_replayed":false,"catalogue_activated":false})
    .to_string();
    if evidence.len() > 33554432 {
        return Err("reconciliation_evidence_capacity");
    }
    client.execute("INSERT INTO brassclaw_monty_review_reconciliations(attempt_id,evidence_bytes,evidence_checksum)
        VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&attempt,&evidence,&checksum(&evidence)])
        .await.map_err(|_| "reconciliation_storage_unavailable")?;
    let recorded: String = client
        .query_one(
            "SELECT evidence_bytes FROM brassclaw_monty_review_reconciliations WHERE attempt_id=$1",
            &[&attempt],
        )
        .await
        .map_err(|_| "reconciliation_storage_unavailable")?
        .get(0);
    Ok(recorded == evidence)
}
