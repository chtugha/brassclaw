//! Read an observed verdict only after the original native task settled.
//! This is receipt authentication, not a second validator or approval producer.
use std::collections::BTreeSet;

use brassclaw_monty_host::service::TaskOutcome;
use brassclaw_skills::value_contract::strict_json;
use serde_json::{Value, json};

use super::{
    Digest, PgPool, REVISION_LIMITS, RetainedValidatorSelection, RetainedValidatorSubject, Sha256,
    Uuid, ValidatorQ1EvidenceError, db, invalid,
};
use crate::global_monty_driver::SettledInternalCompletion;

pub(crate) struct SettledValidatorQ1Verdict {
    pub(crate) succeeded: bool,
    pub(crate) errors: Vec<String>,
}

/// The completion proof is issued after root/child/host-future settlement and
/// the exact factory acknowledgement. Persisted observations, a result string
/// or a caller-provided task ID cannot replace it. Read only the exact selected
/// validator and actual task; never select the latest evidence for a subject.
pub(crate) async fn read_settled_validator_q1(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    selection: &RetainedValidatorSelection,
    completion: &SettledInternalCompletion,
) -> Result<SettledValidatorQ1Verdict, ValidatorQ1EvidenceError> {
    if selection.submission_id() != subject.submission_id()
        || selection.subject_checksum() != subject.subject_checksum()
    {
        return Err(invalid(
            "settled validator subject differs from its selection",
        ));
    }
    let receipt = completion.receipt();
    let TaskOutcome::InternalCompleted { receipt_ref } = &receipt.outcome else {
        return Err(invalid("settled validator has no internal completion"));
    };
    let account = receipt
        .accounting
        .as_ref()
        .filter(|account| {
            account.failure.is_none()
                && account.compute_time.is_some()
                && account.effective_revision > 0
        })
        .ok_or_else(|| invalid("settled validator has no complete task account"))?;
    if !receipt.withheld.is_empty() {
        return Err(invalid("settled validator has unresolved answers"));
    }
    let task = Uuid::parse_str(&account.task.to_string())
        .map_err(|_| invalid("settled validator task identity is invalid"))?;
    let attempt = completion.attempt();
    let reference = crate::monty_result_reference::result_reference(attempt.run_id, receipt_ref)
        .ok_or_else(|| invalid("settled validator result reference is invalid"))?;
    let selection_checksum = hex::encode(Sha256::digest(selection.validator_bytes().as_bytes()));
    let class_code = i16::try_from(subject.candidate().class_code)
        .map_err(|_| invalid("settled validator class is out of range"))?;

    let client = pool
        .get()
        .await
        .map_err(ValidatorQ1EvidenceError::Connection)?;
    let row = client.query_opt(
        "SELECT r.evidence_id,r.succeeded,e.evidence_bytes,e.checksum,a.outcome,a.claim_checksum
         FROM reborn_validator_q1_runs r
         JOIN reborn_component_review_evidence e ON e.evidence_id=r.evidence_id
         JOIN brassclaw_monty_task_admissions a ON a.run_id=$3 AND a.runner_id=$4
         WHERE r.submission_id=$1 AND r.validator_selection_checksum=$2
           AND r.task_handle=$5 AND r.candidate_class_code=$6 AND a.phase='settled'",
        &[&subject.submission_id(), &selection_checksum, &attempt.run_id.as_uuid(),
          &attempt.runner_id.as_uuid(), &task, &class_code],
    ).await.map_err(db)?.ok_or_else(|| invalid("original validator settlement or evidence is missing"))?;
    let claim = serde_json::to_vec(&attempt.lease_token)
        .map_err(|_| invalid("settled validator claim encoding failed"))?;
    if row.get::<_, Vec<u8>>(5) != Sha256::digest(claim).as_slice() {
        return Err(invalid("settled validator claim differs"));
    }
    let outcome: Value = row.get(4);
    let report = &outcome["execution"];
    let root = json!({"format":"monty-root-execution/1",
        "vm_id":receipt.root.vm_id(),
        "source_checksum":hex::encode(receipt.root.source_checksum()),
        "aliases_checksum":hex::encode(receipt.root.aliases_checksum()),
        "workers":receipt.root.workers()});
    let accounting = json!({"effective_revision":account.effective_revision,
        "compute_time":account.compute_time,"failure":account.failure});
    if outcome["status"] != "completed"
        || outcome["result_ref"] != reference.as_str()
        || outcome["internal_receipt"] != receipt_ref.as_str()
        || report["format"] != "monty-internal-task-execution/1"
        || report["root"] != root
        || report["accounting"] != accounting
        || report["root_completed"] != true
        || report["all_selected_recipes_complete"] != true
        || report["internal_receipt"] != receipt_ref.as_str()
        || !report["reply"].is_null()
        || report["withheld_root_answers"] != 0
        || report["withheld_host_answers"] != false
        || report["semantic_approval"] != false
        || report["catalogue_activation"] != false
        || report["recipes"].as_array().is_none_or(|recipes| {
            recipes.len() != 1
                || recipes[0]["selection_checksum"] != selection_checksum
                || recipes[0]["complete"] != true
        })
    {
        return Err(invalid(
            "durable validator settlement differs from the native receipt",
        ));
    }

    let evidence_bytes: &str = row.get(2);
    if hex::encode(Sha256::digest(evidence_bytes.as_bytes())) != row.get::<_, &str>(3) {
        return Err(ValidatorQ1EvidenceError::Integrity);
    }
    let evidence = strict_json(evidence_bytes, REVISION_LIMITS)
        .map_err(|_| invalid("validator evidence violates the strict data contract"))?;
    let fields = [
        "format",
        "evidence_id",
        "kind",
        "validation_mode",
        "submission_id",
        "subject_checksum",
        "validator_selection_checksum",
        "class_code",
        "task_handle",
        "succeeded",
        "reviewed_evidence",
        "report",
    ];
    let succeeded: bool = row.get(1);
    if evidence.as_object().is_none_or(|object| {
        object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field))
    }) || evidence["format"] != "component-review-evidence/1"
        || evidence["evidence_id"] != row.get::<_, Uuid>(0).to_string()
        || evidence["kind"] != "q1"
        || evidence["validation_mode"] != "authored"
        || evidence["submission_id"] != subject.submission_id().to_string()
        || evidence["subject_checksum"] != subject.subject_checksum()
        || evidence["validator_selection_checksum"] != selection_checksum
        || evidence["class_code"] != subject.candidate().class_code
        || evidence["task_handle"] != task.to_string()
        || evidence["succeeded"] != succeeded
        || evidence["reviewed_evidence"] != json!([])
    {
        return Err(invalid("validator evidence identity or outcome differs"));
    }
    let observed = &evidence["report"];
    if observed["format"] != "observed-subject-validator-checks/1"
        || observed["scope"] != "subject-bound-validator-execution"
        || observed["submission_id"] != subject.submission_id().to_string()
        || observed["subject_checksum"] != subject.subject_checksum()
        || observed["selection_checksum"] != selection_checksum
        || observed["class_code"] != subject.candidate().class_code
        || observed["task_handle"] != task.to_string()
        || observed["semantic_approval"] != false
        || observed["q1_qualification"] != false
        || observed["aggregation"]["pass"] != succeeded
    {
        return Err(invalid(
            "validator observations differ from the original execution",
        ));
    }
    Ok(SettledValidatorQ1Verdict {
        succeeded,
        errors: diagnostics(&observed["aggregation"], succeeded)?,
    })
}

fn diagnostics(
    aggregation: &Value,
    succeeded: bool,
) -> Result<Vec<String>, ValidatorQ1EvidenceError> {
    if aggregation
        .as_object()
        .is_none_or(|object| object.len() != 2)
        || aggregation["pass"] != succeeded
    {
        return Err(invalid("validator aggregate shape differs"));
    }
    let rows = aggregation["errors"]
        .as_array()
        .ok_or_else(|| invalid("validator diagnostics are missing"))?;
    if rows.is_empty() != succeeded {
        return Err(invalid("validator diagnostics contradict its outcome"));
    }
    let mut seen = BTreeSet::new();
    let mut errors = Vec::new();
    for row in rows {
        let rule = row["rule_id"]
            .as_str()
            .filter(|rule| !rule.is_empty())
            .ok_or_else(|| invalid("validator diagnostic rule is invalid"))?;
        let messages = row["errors"]
            .as_array()
            .filter(|errors| !errors.is_empty())
            .ok_or_else(|| invalid("validator diagnostic messages are missing"))?;
        if row.as_object().is_none_or(|object| object.len() != 2) || !seen.insert(rule) {
            return Err(invalid(
                "validator diagnostic rule is repeated or malformed",
            ));
        }
        for message in messages {
            let message = message
                .as_str()
                .filter(|message| !message.is_empty())
                .ok_or_else(|| invalid("validator diagnostic message is invalid"))?;
            errors.push(format!("{rule}: {message}"));
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_preserve_the_observed_rule_and_every_message() {
        let aggregate = json!({"pass":false,"errors":[
            {"rule_id":"class-22-structure","errors":["content is empty","export missing"]}
        ]});
        assert_eq!(
            diagnostics(&aggregate, false).unwrap(),
            [
                "class-22-structure: content is empty",
                "class-22-structure: export missing"
            ]
        );
        assert!(
            diagnostics(&json!({"pass":true,"errors":[]}), true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn diagnostics_reject_contradiction_duplicate_rules_and_invalid_messages() {
        for aggregate in [
            json!({"pass":false,"errors":[]}),
            json!({"pass":false,"errors":[{"rule_id":"r","errors":[null]}]}),
            json!({"pass":false,"errors":[{"rule_id":"r","errors":[]}]}),
            json!({"pass":false,"errors":[{"rule_id":"r","errors":[""]}]}),
            json!({"pass":false,"errors":[
                {"rule_id":"r","errors":["first"]},{"rule_id":"r","errors":["second"]}]}),
        ] {
            assert!(diagnostics(&aggregate, false).is_err(), "{aggregate}");
        }
        assert!(
            diagnostics(
                &json!({"pass":true,"errors":[
                    {"rule_id":"r","errors":["failed"]}
                ]}),
                true
            )
            .is_err()
        );
    }
}
