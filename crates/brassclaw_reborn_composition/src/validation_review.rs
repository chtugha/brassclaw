//! Exact legacy Q1 candidate retention. This is not v3 combination approval.
use super::{
    ComponentScope, STATE_Q1_PENDING, ValidationQueueError, ValidationQueueStore, map_pg, map_pool,
    resolve_component_table, resolve_content_column, validate_upgrade_payload,
};
use serde_json::Value;
use uuid::Uuid;

// Exclude only lifecycle/accounting fields which may change without changing
// authoring meaning. New columns are protected by default. Processing status,
// consumer tags, contracts, binding metadata and content hashes stay included.
const COMPONENT_VOLATILE: &str = "ARRAY['created_at','updated_at','usage_count','success_count','failure_count','wilson_lower','tier','confidence','last_audit_at','audit_failure_count','validation_status','validation_errors','review_feedback','review_attempts','rejected_at','queue_code']::text[]";
pub(super) const QUEUE_VOLATILE: &str = "ARRAY['state','validation_errors','updated_at','q2_actor','q1_component_bytes','q1_component_checksum','q1_queue_bytes','q1_queue_checksum']::text[]";

/// Constructed only by a locked read of an actual queued candidate. No Debug:
/// retained candidate/queue bytes may include private authored content.
pub(crate) struct Q1Candidate {
    component_id: Uuid,
    class_code: i16,
    component_bytes: String,
    queue_bytes: String,
    reviewed: Value,
}
impl Q1Candidate {
    pub(crate) fn class_code(&self) -> i16 {
        self.class_code
    }
    pub(crate) fn reviewed(&self) -> &Value {
        &self.reviewed
    }
}

pub(super) async fn component_bytes(
    tx: &tokio_postgres::Transaction<'_>,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i16,
) -> Result<String, ValidationQueueError> {
    let table = resolve_component_table(i32::from(class_code)).ok_or(
        ValidationQueueError::UnknownClass {
            class_code: i32::from(class_code),
        },
    )?;
    let sql = format!(
        "SELECT (to_jsonb(c) - {COMPONENT_VOLATILE})::text FROM {table} c
         WHERE id=$1 AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5
         AND class_code=$6 FOR UPDATE"
    );
    tx.query_opt(
        &sql,
        &[
            &component_id,
            &scope.tenant_id,
            &scope.user_id,
            &scope.agent_id,
            &scope.project_id,
            &class_code,
        ],
    )
    .await
    .map_err(map_pg)?
    .map(|row| row.get(0))
    .ok_or(ValidationQueueError::ComponentMissing { component_id })
}

impl ValidationQueueStore {
    /// Take one queue-before-component snapshot, then release all locks before
    /// parsing or executing validator code. Graduation compares it again.
    pub(crate) async fn capture_q1_candidate(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
    ) -> Result<Q1Candidate, ValidationQueueError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;
        let sql = format!(
            "SELECT state, component_class, proposed_payload, (to_jsonb(q)-{QUEUE_VOLATILE})::text
             FROM reborn_validation_queue q WHERE tenant_id=$1 AND user_id=$2
             AND agent_id=$3 AND project_id=$4 AND component_id=$5 FOR UPDATE"
        );
        let row = tx
            .query_opt(
                &sql,
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                ],
            )
            .await
            .map_err(map_pg)?
            .ok_or(ValidationQueueError::NotFound { component_id })?;
        if row.get::<_, i16>(0) != STATE_Q1_PENDING {
            return Err(ValidationQueueError::NotFound { component_id });
        }
        let class_code: i16 = row.get(1);
        let proposal: Option<Value> = row.get(2);
        if let Some(payload) = &proposal {
            validate_upgrade_payload(i32::from(class_code), component_id, payload)?;
        }
        let queue_bytes: String = row.get(3);
        let component_bytes = component_bytes(&tx, scope, component_id, class_code).await?;
        let mut reviewed: Value = serde_json::from_str(&component_bytes)
            .map_err(|_| ValidationQueueError::InvalidPayload { component_id })?;
        let fields = reviewed
            .as_object_mut()
            .ok_or(ValidationQueueError::InvalidPayload { component_id })?;
        if let Some(proposal) = proposal {
            for name in ["name", "description"] {
                if let Some(value) = proposal.get(name) {
                    fields.insert(name.to_owned(), value.clone());
                }
            }
            if let Some(value) = proposal.get("content") {
                let (column, _) = resolve_content_column(class_code)
                    .ok_or(ValidationQueueError::InvalidPayload { component_id })?;
                fields.insert(column.to_owned(), value.clone());
            }
        }
        tx.commit().await.map_err(map_pg)?;
        Ok(Q1Candidate {
            component_id,
            class_code,
            component_bytes,
            queue_bytes,
            reviewed,
        })
    }

    /// The sole production state-2 writer. An unchanged current row does not
    /// suffice: the candidate must equal the snapshot supplied to the validator.
    pub(crate) async fn gate1_pass_reviewed(
        &self,
        scope: &ComponentScope,
        review: &Q1Candidate,
        errors: &[String],
    ) -> Result<(), ValidationQueueError> {
        let component_id = review.component_id;
        if !errors.is_empty() {
            return Err(ValidationQueueError::InvalidGate1Result { component_id });
        }
        self.record_q1_review(scope, review, errors, true).await
    }

    pub(crate) async fn gate1_fail_reviewed(
        &self,
        scope: &ComponentScope,
        review: &Q1Candidate,
        errors: &[String],
    ) -> Result<(), ValidationQueueError> {
        if errors.is_empty() {
            return Err(ValidationQueueError::InvalidGate1Result {
                component_id: review.component_id,
            });
        }
        self.record_q1_review(scope, review, errors, false).await
    }

    async fn record_q1_review(
        &self,
        scope: &ComponentScope,
        review: &Q1Candidate,
        errors: &[String],
        passed: bool,
    ) -> Result<(), ValidationQueueError> {
        let component_id = review.component_id;
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;
        let sql = format!(
            "SELECT state, (to_jsonb(q)-{QUEUE_VOLATILE})::text FROM reborn_validation_queue q
             WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
             AND component_id=$5 FOR UPDATE"
        );
        let row = tx
            .query_opt(
                &sql,
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                ],
            )
            .await
            .map_err(map_pg)?
            .ok_or(ValidationQueueError::NotFound { component_id })?;
        if row.get::<_, i16>(0) != STATE_Q1_PENDING {
            return Err(ValidationQueueError::NotFound { component_id });
        }
        if row.get::<_, String>(1) != review.queue_bytes
            || component_bytes(&tx, scope, component_id, review.class_code).await?
                != review.component_bytes
        {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }
        let n = tx.execute(
            "UPDATE reborn_validation_queue SET state=$8, validation_errors=$9, updated_at=now(),
             q1_component_bytes=$6, q1_component_checksum=encode(sha256(convert_to($6::text,'UTF8')),'hex'),
             q1_queue_bytes=$7, q1_queue_checksum=encode(sha256(convert_to($7::text,'UTF8')),'hex')
             WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 AND state=1",
            &[&scope.tenant_id, &scope.user_id, &scope.agent_id, &scope.project_id,
                &component_id, &review.component_bytes, &review.queue_bytes,
                &(if passed { 2_i16 } else { 1_i16 }), &errors.to_vec()],
        ).await.map_err(map_pg)?;
        if n != 1 {
            return Err(ValidationQueueError::NotFound { component_id });
        }
        tx.commit().await.map_err(map_pg)?;
        Ok(())
    }

    // Queue lifecycle fixtures intentionally bypass structural execution, but
    // must still inspect an actual existing candidate and satisfy its seal.
    #[cfg(test)]
    pub(crate) async fn gate1_pass(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        errors: &[String],
    ) -> Result<(), ValidationQueueError> {
        if !errors.is_empty() {
            return Err(ValidationQueueError::InvalidGate1Result { component_id });
        }
        let review = self.capture_q1_candidate(scope, component_id).await?;
        self.gate1_pass_reviewed(scope, &review, errors).await
    }
}
