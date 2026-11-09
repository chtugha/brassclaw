//! Exact legacy Q1 candidate retention. This is not v3 combination approval.
#[cfg(test)]
use super::map_pool;
use super::{
    ComponentScope, STATE_Q1_PENDING, ValidationQueueError, ValidationQueueStore,
    map_original_store, map_pg, resolve_component_table, resolve_content_column,
    validate_upgrade_payload,
};
use serde_json::Value;
use uuid::Uuid;

// Exclude only lifecycle/accounting fields which may change without changing
// authoring meaning. New columns are protected by default. Processing status,
// consumer tags, contracts, binding metadata and content hashes stay included.
pub(super) const QUEUE_VOLATILE: &str = "ARRAY['state','validation_errors','updated_at','q2_actor','q1_component_bytes','q1_component_checksum','q1_queue_bytes','q1_queue_checksum']::text[]";

/// Constructed only by a locked read of an actual queued candidate. No Debug:
/// retained candidate/queue bytes may include private authored content.
pub(crate) struct Q1Candidate {
    component_id: Uuid,
    class_code: i16,
    component_bytes: String,
    queue_bytes: String,
    reviewed: Value,
    queue_id: Uuid,
    original_binding_required: bool,
    binding: Option<brassclaw_skills::review_submission_store::queue::RetainedQueueReview>,
}
impl Q1Candidate {
    pub(crate) fn original(
        &self,
    ) -> Result<
        &brassclaw_skills::review_submission_store::queue::RetainedQueueReview,
        ValidationQueueError,
    > {
        self.binding.as_ref().ok_or({
            if self.original_binding_required {
                ValidationQueueError::OriginalReviewUnresolved {
                    component_id: self.component_id,
                }
            } else {
                ValidationQueueError::LegacyReviewCustodyUnresolved {
                    component_id: self.component_id,
                }
            }
        })
    }
    #[cfg(test)]
    pub(crate) fn class_code(&self) -> i16 {
        self.class_code
    }
    #[cfg(test)]
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
    component_bytes_on(tx, scope, component_id, class_code, true).await
}

async fn component_bytes_on(
    tx: &tokio_postgres::Transaction<'_>,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i16,
    retain_lock: bool,
) -> Result<String, ValidationQueueError> {
    let table = resolve_component_table(i32::from(class_code)).ok_or(
        ValidationQueueError::UnknownClass {
            class_code: i32::from(class_code),
        },
    )?;
    let mut sql = format!(
        "SELECT review_component_bytes(to_jsonb(c)) FROM {table} c
         WHERE id=$1 AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5
         AND class_code=$6"
    );
    if retain_lock {
        sql.push_str(" FOR UPDATE");
    }
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
    /// Read projection only. Historical acknowledged applications do not call
    /// this: subsequent legitimate edits/deletions cannot rewrite their result.
    pub(crate) async fn original_queue_is_current(
        tx: &tokio_postgres::Transaction<'_>,
        submission_id: Uuid,
    ) -> Result<bool, ValidationQueueError> {
        let rows = tx
            .query(
                "SELECT b.queue_id FROM reborn_component_review_queue_bindings b
            WHERE b.submission_id=$1 ORDER BY b.queue_id",
                &[&submission_id],
            )
            .await
            .map_err(map_pg)?;
        for row in rows {
            let queue_id: Uuid = row.get(0);
            let original =
                brassclaw_skills::review_submission_store::queue::read_in_transaction(tx, queue_id)
                    .await
                    .map_err(|error| map_original_store(error, Uuid::nil()))?
                    .ok_or(ValidationQueueError::InvalidPayload {
                        component_id: Uuid::nil(),
                    })?;
            let Some(current)=tx.query_opt("SELECT tenant_id,user_id,agent_id,project_id,component_id,component_class,state,review_queue_bytes(q)
                FROM reborn_validation_queue q WHERE id=$1",&[&queue_id]).await.map_err(map_pg)? else {return Ok(false);};
            if current.get::<_, i16>(6) != STATE_Q1_PENDING
                || current.get::<_, &str>(7) != original.queue_bytes()
            {
                return Ok(false);
            }
            let scope = ComponentScope {
                tenant_id: current.get(0),
                user_id: current.get(1),
                agent_id: current.get(2),
                project_id: current.get(3),
            };
            match component_bytes_on(tx, &scope, current.get(4), current.get(5), false).await {
                Ok(bytes) if bytes == original.component_bytes() => {}
                Ok(_) | Err(ValidationQueueError::ComponentMissing { .. }) => return Ok(false),
                Err(error) => return Err(error),
            }
        }
        Ok(true)
    }
    /// Take one queue-before-component snapshot, then release all locks before
    /// parsing or executing validator code. Graduation compares it again.
    #[cfg(test)]
    pub(crate) async fn capture_q1_candidate(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
    ) -> Result<Q1Candidate, ValidationQueueError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(map_pg)?;
        let review = Self::capture_candidate_in_transaction(&tx, scope, component_id).await?;
        tx.commit().await.map_err(map_pg)?;
        Ok(review)
    }

    pub(super) async fn capture_candidate_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        scope: &ComponentScope,
        component_id: Uuid,
    ) -> Result<Q1Candidate, ValidationQueueError> {
        let sql = String::from(
            "SELECT state, component_class, proposed_payload, review_queue_bytes(q), id, original_binding_required
             FROM reborn_validation_queue q WHERE tenant_id=$1 AND user_id=$2
             AND agent_id=$3 AND project_id=$4 AND component_id=$5 FOR UPDATE",
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
        let queue_id: Uuid = row.get(4);
        let component_bytes = component_bytes(tx, scope, component_id, class_code).await?;
        let mut reviewed: Value = serde_json::from_str(&component_bytes)
            .map_err(|_| ValidationQueueError::InvalidPayload { component_id })?;
        if matches!(class_code, 0 | 1..=3 | 13 | 21..=23) {
            // The retained projection/overlay has one SQL owner, reused by the
            // capture precheck, immutable reader and independent insert trigger.
            reviewed = tx
                .query_one(
                    "SELECT review_queue_proposed($1::smallint,$2::jsonb,$3::jsonb)",
                    &[&class_code, &reviewed, &proposal],
                )
                .await
                .map_err(map_pg)?
                .get(0);
        } else {
            // Preserve the existing non-retained document/note lifecycle.
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
        }
        let binding =
            brassclaw_skills::review_submission_store::queue::read_in_transaction(tx, queue_id)
                .await
                .map_err(|error| map_original_store(error, component_id))?;
        if let Some(binding) = &binding
            && (binding.component_bytes() != component_bytes
                || binding.queue_bytes() != queue_bytes)
        {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }
        Ok(Q1Candidate {
            component_id,
            class_code,
            component_bytes,
            queue_bytes,
            reviewed,
            queue_id,
            original_binding_required: row.get(5),
            binding,
        })
    }

    /// Legacy controlled-test state-2 writer. An unchanged current row does not
    /// suffice: the candidate must equal the snapshot supplied to the validator.
    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
    async fn record_q1_review(
        &self,
        scope: &ComponentScope,
        review: &Q1Candidate,
        errors: &[String],
        passed: bool,
    ) -> Result<(), ValidationQueueError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(map_pg)?;
        Self::record_q1_review_in_transaction(&tx, scope, review, errors, passed).await?;
        tx.commit().await.map_err(map_pg)?;
        Ok(())
    }

    pub(crate) async fn record_structural_failure_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        scope: &ComponentScope,
        review: &Q1Candidate,
        errors: &[String],
    ) -> Result<(), ValidationQueueError> {
        if errors.is_empty() {
            return Err(ValidationQueueError::InvalidGate1Result {
                component_id: review.component_id,
            });
        }
        Self::record_q1_review_in_transaction(tx, scope, review, errors, false).await
    }

    async fn record_q1_review_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        scope: &ComponentScope,
        review: &Q1Candidate,
        errors: &[String],
        passed: bool,
    ) -> Result<(), ValidationQueueError> {
        let component_id = review.component_id;
        let sql = String::from(
            "SELECT state, review_queue_bytes(q), id FROM reborn_validation_queue q
             WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
             AND component_id=$5 FOR UPDATE",
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
            || component_bytes(tx, scope, component_id, review.class_code).await?
                != review.component_bytes
        {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }
        if matches!(review.class_code,0|1..=3|13|21..=23) {
            let original = review.original()?;
            Self::verify_original_in_transaction(tx, row.get(2), original).await?;
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
        Ok(())
    }

    pub(super) async fn verify_original_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        queue_id: Uuid,
        original: &brassclaw_skills::review_submission_store::queue::RetainedQueueReview,
    ) -> Result<(), ValidationQueueError> {
        let component_id = original
            .candidate()
            .map_err(|_| ValidationQueueError::InvalidPayload {
                component_id: Uuid::nil(),
            })?
            .uuid;
        let current =
            brassclaw_skills::review_submission_store::queue::read_in_transaction(tx, queue_id)
                .await
                .map_err(|error| map_original_store(error, component_id))?
                .ok_or(ValidationQueueError::OriginalReviewUnresolved { component_id })?;
        let subject=brassclaw_skills::review_submission_store::PgReviewSubmissionStore::read_in_transaction(tx,current.submission_id()).await
            .map_err(|error| map_original_store(error, component_id))?.ok_or(ValidationQueueError::ReviewChanged{component_id})?;
        if subject.function_partner.is_some() {
            return Err(ValidationQueueError::OriginalReviewUnresolved { component_id });
        }
        if current.checksum() != original.checksum() || current.queue_id() != original.queue_id() {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }
        Ok(())
    }

    pub(crate) async fn read_original_create(
        tx: &tokio_postgres::Transaction<'_>,
        draft: &brassclaw_skills::review_submission_store::ReviewSubmissionDraft,
        scope: &ComponentScope,
    ) -> Result<
        Option<brassclaw_skills::review_submission_store::RetainedReviewSubmission>,
        ValidationQueueError,
    > {
        let component_id = draft.candidate().uuid;
        let existing=brassclaw_skills::review_submission_store::PgReviewSubmissionStore::read_in_transaction(tx,draft.submission_id()).await
            .map_err(|error| map_original_store(error, component_id))?;
        if existing.is_none() {
            return Ok(None);
        }
        let ids:Vec<Uuid>=tx.query_opt("SELECT queue_ids,request_kind FROM reborn_component_review_queue_requests WHERE submission_id=$1",&[&draft.submission_id()]).await.map_err(map_pg)?
            .ok_or(ValidationQueueError::ReviewChanged{component_id})?.get(0);
        if ids.len() != 1 {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }
        let original =
            brassclaw_skills::review_submission_store::queue::read_in_transaction(tx, ids[0])
                .await
                .map_err(|error| map_original_store(error, component_id))?
                .ok_or(ValidationQueueError::ReviewChanged { component_id })?;
        let queue: Value = serde_json::from_str(original.queue_bytes())
            .map_err(|_| ValidationQueueError::InvalidPayload { component_id })?;
        if queue["tenant_id"] != scope.tenant_id
            || queue["user_id"] != scope.user_id
            || queue["agent_id"] != scope.agent_id
            || queue["project_id"] != scope.project_id
        {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }
        Self::submit_original_in_transaction(tx, draft, &[], &ids, "create_queue")
            .await
            .map(Some)
    }

    /// Capture all queue roots before the immutable store takes revision-head
    /// locks. This method performs no Monty work and joins the author transaction.
    pub(crate) async fn submit_original_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        draft: &brassclaw_skills::review_submission_store::ReviewSubmissionDraft,
        targets: &[(ComponentScope, Uuid, Uuid)],
        queue_ids: &[Uuid],
        request_kind: &str,
    ) -> Result<
        brassclaw_skills::review_submission_store::RetainedReviewSubmission,
        ValidationQueueError,
    > {
        let component_id = draft.candidate().uuid;
        let mut original_ids = queue_ids.to_vec();
        original_ids.sort();
        if original_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ValidationQueueError::InvalidPayload { component_id });
        }
        if brassclaw_skills::review_submission_store::PgReviewSubmissionStore::read_in_transaction(
            tx,
            draft.submission_id(),
        )
        .await
        .map_err(|error| map_original_store(error, component_id))?
        .is_some()
        {
            // Verify identical subject bytes before returning any original ACK.
            let subject=brassclaw_skills::review_submission_store::PgReviewSubmissionStore::submit_in_transaction(tx,draft).await
                .map_err(|error| map_original_store(error, component_id))?;
            let old=tx.query_opt("SELECT queue_ids,request_kind FROM reborn_component_review_queue_requests WHERE submission_id=$1",&[&draft.submission_id()]).await.map_err(map_pg)?;
            match old {
                Some(row)
                    if row.get::<_, Vec<Uuid>>(0) == original_ids
                        && row.get::<_, String>(1) == request_kind =>
                {
                    for queue_id in &original_ids {
                        let binding =
                            brassclaw_skills::review_submission_store::queue::read_in_transaction(
                                tx, *queue_id,
                            )
                            .await
                            .map_err(|error| map_original_store(error, component_id))?
                            .ok_or(ValidationQueueError::ReviewChanged { component_id })?;
                        if binding.submission_id() != subject.id
                            || binding.subject_checksum() != subject.checksum
                        {
                            return Err(ValidationQueueError::ReviewChanged { component_id });
                        }
                    }
                    return Ok(subject);
                }
                None if original_ids.is_empty() && request_kind == "standalone" => {
                    return Ok(subject);
                }
                _ => return Err(ValidationQueueError::ReviewChanged { component_id }),
            }
        }
        let roots = draft
            .changed_roots()
            .map_err(|_| ValidationQueueError::InvalidPayload {
                component_id: draft.candidate().uuid,
            })?;
        let mut targets = targets.to_vec();
        targets.sort_by_key(|(_, id, _)| *id);
        if targets.windows(2).any(|pair| pair[0].1 == pair[1].1)
            || targets
                .iter()
                .any(|(_, id, _)| !roots.iter().any(|r| r.uuid == *id))
            || (!targets.is_empty() && targets.len() != roots.len())
        {
            return Err(ValidationQueueError::InvalidPayload {
                component_id: draft.candidate().uuid,
            });
        }
        for (scope, id, _) in &targets {
            Self::lock_queue_key(tx, scope, *id).await?;
        }
        // Lock every queue before any component, including joint submissions.
        for (scope, id, _) in &targets {
            tx.query_opt("SELECT id FROM reborn_validation_queue WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 FOR UPDATE",
                &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,id]).await.map_err(map_pg)?
                .ok_or(ValidationQueueError::NotFound{component_id:*id})?;
        }
        let mut captures = Vec::new();
        for (scope, id, queue_id) in &targets {
            let capture = Self::capture_candidate_in_transaction(tx, scope, *id).await?;
            if !capture.original_binding_required {
                return Err(ValidationQueueError::LegacyReviewCustodyUnresolved {
                    component_id: *id,
                });
            }
            if capture.queue_id != *queue_id {
                return Err(ValidationQueueError::ReviewChanged { component_id: *id });
            }
            captures.push(capture);
        }
        // Validate the complete original projection before allocating revisions.
        // SQL owns the one field/agreement contract; the insert trigger repeats
        // it against independently loaded original bytes and locked actual rows.
        if !captures.is_empty() {
            let documents = draft
                .changed_root_documents()
                .map_err(|error| map_original_store(error, component_id))?;
            for capture in &captures {
                let (reference, document) = documents
                    .iter()
                    .find(|(r, _)| r.uuid == capture.component_id)
                    .ok_or(ValidationQueueError::InvalidPayload {
                        component_id: capture.component_id,
                    })?;
                let class = i16::try_from(reference.class_code).map_err(|_| {
                    ValidationQueueError::InvalidPayload {
                        component_id: capture.component_id,
                    }
                })?;
                let agrees: Option<bool> = tx
                    .query_one(
                        "SELECT review_queue_agrees($1::smallint,$2::jsonb,$3::jsonb)",
                        &[&class, document, &capture.reviewed],
                    )
                    .await
                    .map_err(map_pg)?
                    .get(0);
                if agrees != Some(true) {
                    return Err(ValidationQueueError::ReviewChanged {
                        component_id: capture.component_id,
                    });
                }
            }
        }
        let subject=brassclaw_skills::review_submission_store::PgReviewSubmissionStore::submit_in_transaction(tx,draft).await
            .map_err(|error|map_original_store(error,draft.candidate().uuid))?;
        tx.execute("INSERT INTO reborn_component_review_queue_requests(submission_id,queue_ids,request_kind) VALUES($1,$2,$3)",
            &[&subject.id,&original_ids,&request_kind]).await.map_err(map_pg)?;
        for capture in captures {
            brassclaw_skills::review_submission_store::queue::retain_in_transaction(
                tx,
                &subject,
                capture.queue_id,
                capture.component_id,
                capture.component_bytes,
                capture.queue_bytes,
                capture.reviewed.to_string(),
            )
            .await
            .map_err(|error| map_original_store(error, capture.component_id))?;
        }
        Ok(subject)
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
        let empty_code_fixture = review.class_code == 22
            && review.reviewed.get("includes") == Some(&serde_json::json!([]))
            && review.reviewed.get("dependency_registry") == Some(&Value::Null);
        if (review.class_code == 21 || empty_code_fixture) && review.binding.is_none() {
            use brassclaw_skills::{
                component_revision::ComponentRevisionDraft,
                review_submission_store::ReviewSubmissionDraft,
                revision_store::PgComponentRevisionStore,
            };
            // Existing Recipe and explicitly dependency-free PythonCode lifecycle
            // fixtures declare no retained graph. This test-only helper cannot
            // reconstruct dependencies/Skill associations or certify executable Q1.
            let revision = |document: Value| {
                ComponentRevisionDraft::from_json(
                    &serde_json::json!({
                        "format":"component-revision/1","uuid":component_id,"class_code":review.class_code,
                        "document":document,"dependencies":[],"association":null,
                    })
                    .to_string(),
                )
                .map_err(|_| ValidationQueueError::InvalidPayload { component_id })
            };
            let candidate = revision(review.reviewed)?;
            let mut client = self.pool.get().await.map_err(map_pool)?;
            let tx = client
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .start()
                .await
                .map_err(map_pg)?;
            Self::lock_queue_key(&tx, scope, component_id).await?;
            let current = Self::capture_candidate_in_transaction(&tx, scope, component_id).await?;
            let queue: Value = serde_json::from_str(&current.queue_bytes)
                .map_err(|_| ValidationQueueError::InvalidPayload { component_id })?;
            let base = if !queue["proposed_payload"].is_null() {
                let document = serde_json::from_str(&current.component_bytes)
                    .map_err(|_| ValidationQueueError::InvalidPayload { component_id })?;
                Some(
                    PgComponentRevisionStore::stage_in_transaction(&tx, &revision(document)?, 0)
                        .await
                        .map_err(|error| map_original_store(error.into(), component_id))?,
                )
            } else {
                None
            };
            let draft = ReviewSubmissionDraft::new(
                Uuid::new_v4(),
                "queue-lifecycle-fixture",
                candidate,
                base,
                vec![],
            )
            .map_err(|_| ValidationQueueError::InvalidPayload { component_id })?;
            Self::submit_original_in_transaction(
                &tx,
                &draft,
                &[(scope.clone(), component_id, review.queue_id)],
                &[review.queue_id],
                "existing_queue",
            )
            .await?;
            tx.commit().await.map_err(map_pg)?;
        }
        let review = self.capture_q1_candidate(scope, component_id).await?;
        self.gate1_pass_reviewed(scope, &review, errors).await
    }
}
