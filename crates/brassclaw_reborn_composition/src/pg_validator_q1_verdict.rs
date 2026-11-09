//! Read an observed verdict only after the original native task settled.
//! This is receipt authentication, not a second validator or approval producer.
use std::collections::BTreeSet;

use brassclaw_skills::value_contract::strict_json;
use serde_json::{Value, json};

use super::{
    Digest, REVISION_LIMITS, RetainedValidatorSelection, RetainedValidatorSubject, Sha256, Uuid,
    ValidatorQ1EvidenceError, db, invalid,
};
#[cfg(test)]
use crate::global_monty_driver::SettledInternalCompletion;
#[cfg(test)]
use brassclaw_pg::PgPool;

pub(crate) struct SettledValidatorQ1Verdict {
    pub(crate) succeeded: bool,
    pub(crate) errors: Vec<String>,
    pub(crate) evidence_id: Uuid,
    pub(crate) evidence_checksum: String,
}

/// The completion proof is issued after root/child/host-future settlement and
/// the exact factory acknowledgement. Persisted observations, a result string
/// or a caller-provided task ID cannot replace it. Read only the exact selected
/// validator and actual task; never select the latest evidence for a subject.
#[cfg(test)]
pub(crate) async fn read_settled_validator_q1(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    selection: &RetainedValidatorSelection,
    completion: &SettledInternalCompletion,
) -> Result<SettledValidatorQ1Verdict, ValidatorQ1EvidenceError> {
    let mut client = pool
        .get()
        .await
        .map_err(ValidatorQ1EvidenceError::Connection)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(db)?;
    let original = crate::pg_monty_admission::PgMontyAdmission::verify_private_live_completion(
        &tx, completion,
    )
    .await
    .map_err(|_| invalid("original private completion differs"))?
    .ok_or_else(|| invalid("validator completion is not private"))?;
    let delivery = crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(
        &tx,
        original.request().work_id(),
        original.request().context(),
    )
    .await
    .map_err(|_| invalid("validator post-Factory delivery invalid"))?
    .ok_or_else(|| invalid("validator post-Factory delivery missing"))?;
    let verdict = read_delivered_validator_q1(&tx, subject, selection, &delivery).await?;
    tx.commit().await.map_err(db)?;
    Ok(verdict)
}

/// Read-only historical authentication. This consumes a native-owner delivery
/// attestation, never reconstructs a live receipt and never reexecutes checks.
pub(crate) async fn read_delivered_validator_q1(
    tx: &tokio_postgres::Transaction<'_>,
    subject: &RetainedValidatorSubject,
    selection: &RetainedValidatorSelection,
    delivery: &crate::pg_monty_admission::DeliveredPrivateCompletion,
) -> Result<SettledValidatorQ1Verdict, ValidatorQ1EvidenceError> {
    use brassclaw_turns::private_work::PrivateOriginalRef;
    let request = delivery.original().request();
    if selection.submission_id() != subject.submission_id()
        || selection.subject_checksum() != subject.subject_checksum()
        || !matches!(request.original(),PrivateOriginalRef::ValidatorStructural{submission_id,subject_checksum}
            if *submission_id==subject.submission_id() && hex::encode(subject_checksum)==subject.subject_checksum())
        || delivery.original().definition().workflow_checksum()
            != <[u8; 32]>::from(Sha256::digest(selection.validator_bytes().as_bytes()))
    {
        return Err(invalid("delivered validator subject differs"));
    }
    crate::pg_review_admission_source::ValidatorInputCapture::verify_historical(
        subject,
        selection,
        delivery.original().definition(),
    )
    .map_err(|_| invalid("original validator input capture differs"))?;
    let task = Uuid::parse_str(&delivery.task().to_string())
        .map_err(|_| invalid("validator task identity invalid"))?;
    let selection_checksum = hex::encode(Sha256::digest(selection.validator_bytes().as_bytes()));
    let class_code = i16::try_from(subject.candidate().class_code)
        .map_err(|_| invalid("validator class invalid"))?;
    let row = tx.query_opt("SELECT r.evidence_id,r.succeeded,e.evidence_bytes,e.checksum
        FROM reborn_validator_q1_runs r JOIN reborn_component_review_evidence e USING(evidence_id)
        WHERE r.submission_id=$1 AND r.validator_selection_checksum=$2 AND r.task_handle=$3 AND r.candidate_class_code=$4",
        &[&subject.submission_id(),&selection_checksum,&task,&class_code]).await.map_err(db)?
        .ok_or_else(|| invalid("original delivered validator evidence missing"))?;
    let report = delivery.report();
    if report["recipes"].as_array().is_none_or(|recipes| {
        recipes.len() != 1
            || recipes[0]["selection_checksum"] != selection_checksum
            || recipes[0]["complete"] != true
    }) {
        return Err(invalid(
            "delivered validator execution differs from selection",
        ));
    }

    let evidence_bytes: &str = row.get(2);
    if hex::encode(Sha256::digest(evidence_bytes.as_bytes())) != row.get::<_, &str>(3) {
        return Err(ValidatorQ1EvidenceError::Integrity);
    }
    let bounds = delivery
        .original()
        .definition()
        .preparation()
        .limits()
        .bounds;
    let evidence_limits = if selection.source_qualification().is_some() {
        brassclaw_skills::value_contract::ContractLimits {
            max_depth: bounds.max_value_depth,
            max_nodes: bounds.max_value_nodes,
            max_bytes: bounds.max_value_bytes,
        }
    } else {
        REVISION_LIMITS
    };
    let evidence = strict_json(evidence_bytes, evidence_limits)
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
        evidence_id: row.get(0),
        evidence_checksum: row.get(3),
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
