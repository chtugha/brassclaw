//! Original private preparations in the existing installed catalogue owner.
//! This captures/validates bytes and bindings; Python executes the Recipe.
use super::*;
use crate::private_work_definition::{
    DefinitionCapture, DomainCapture, PrivateWorkDefinition, checksum, decode_checksum,
};
use brassclaw_turns::{
    PgTurnStateStore, TurnRunId,
    private_work::{
        PrivateOriginalRef, PrivatePreparationReason, PrivateWorkRequest, PrivateWorkStatus,
    },
};

#[cfg(test)]
struct ValidatorOriginal {
    subject: Arc<brassclaw_skills::review_submission_store::RetainedValidatorSubject>,
    selection:
        Arc<brassclaw_skills::review_submission_store::selection::RetainedValidatorSelection>,
    context: brassclaw_turns::private_work::PrivateExecutionContext,
    root: brassclaw_skills::component_revision::RetainedComponentRevision,
}

#[async_trait]
impl brassclaw_turns::pg_turn_state_store::PgPrivateOriginalVerifier for InstalledMontyCatalogue {
    async fn verify_original(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        request: &PrivateWorkRequest,
    ) -> Result<(), brassclaw_turns::TurnError> {
        let error = || brassclaw_turns::TurnError::Unavailable {
            reason: "private domain original mismatch".into(),
        };
        match request.original() {
            PrivateOriginalRef::PrefixSourceOnly {
                job_id,
                definition_checksum,
            } => {
                let definition =
                    crate::prefix_job_definitions::Store::get_in_transaction(tx, *job_id)
                        .await
                        .map_err(|_| error())?
                        .ok_or_else(error)?;
                if !matches!(
                    definition.origin,
                    crate::prefix_job_definitions::Origin::Private { .. }
                ) || definition.operation != "generate"
                    || definition.mode != "source_only"
                    || definition.original_checksum() != *definition_checksum
                    || definition
                        .origin
                        .context(&definition.created_by)
                        .map_err(|_| error())?
                        != *request.context()
                    || request.run_id().as_uuid() != *job_id
                {
                    return Err(error());
                }
            }
            PrivateOriginalRef::ValidatorStructural {
                submission_id,
                subject_checksum,
            } => {
                // Domain bytes/selection/input construction remain A-03's owner.
                let row = tx
                    .query_opt(
                        "SELECT s.actor,s.checksum FROM reborn_component_review_submissions s
                    JOIN reborn_component_review_requests r USING(submission_id)
                    JOIN reborn_component_review_queue_requests q USING(submission_id)
                    WHERE s.submission_id=$1 AND s.checksum=r.subject_checksum",
                        &[submission_id],
                    )
                    .await
                    .map_err(|_| error())?
                    .ok_or_else(error)?;
                if decode_checksum(row.get::<_, &str>(1)).map_err(|_| error())? != *subject_checksum
                    || request
                        .context()
                        .actor
                        .as_ref()
                        .map(|actor| actor.user_id.as_str())
                        != Some(row.get::<_, &str>(0))
                    || self.private_work_context.get().is_none_or(|context| {
                        context.tenant_id != request.context().tenant_id
                            || context.agent_id != request.context().agent_id
                            || context.project_id != request.context().project_id
                    })
                {
                    return Err(error());
                }
            }
        }
        Ok(())
    }
}

impl InstalledMontyCatalogue {
    pub(crate) fn source_work_context(
        &self,
    ) -> Result<brassclaw_turns::private_work::PrivateExecutionContext, RebornBuildError> {
        self.private_work_context
            .get()
            .cloned()
            .ok_or_else(|| invalid("Source Work runtime context unavailable"))
    }
    #[cfg(test)]
    pub(crate) async fn prepare_private_validator(
        &self,
        submission_id: Uuid,
    ) -> Result<brassclaw_turns::private_work::PrivateWorkState, RebornBuildError> {
        use brassclaw_skills::component_revision::REVISION_LIMITS;
        use brassclaw_skills::review_submission_store::{PgReviewSubmissionStore, selection};
        let mut context = self
            .private_work_context
            .get()
            .cloned()
            .ok_or_else(|| invalid("private validator runtime context unavailable"))?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| invalid("private validator preparation unavailable"))?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| invalid("private validator preparation unavailable"))?;
        let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            submission_id,
            REVISION_LIMITS,
        )
        .await
        .map_err(|error| invalid(error.to_string()))?
        .ok_or_else(|| invalid("private validator subject missing"))?;
        let selection =
            selection::read_preparation_in_transaction(&tx, submission_id, REVISION_LIMITS)
                .await
                .map_err(|error| invalid(error.to_string()))?
                .ok_or_else(|| invalid("private validator selection missing"))?;
        let actor = tx
            .query_one(
                "SELECT actor FROM reborn_component_review_submissions WHERE submission_id=$1",
                &[&submission_id],
            )
            .await
            .map_err(|_| invalid("private validator actor unavailable"))?;
        context.actor = Some(brassclaw_turns::TurnActor::new(
            brassclaw_host_api::UserId::new(actor.get::<_, String>(0))
                .map_err(|error| invalid(error.to_string()))?,
        ));
        let root = PgComponentRevisionStore::read_revision_in_transaction(&tx, self.root_reference)
            .await
            .map_err(|error| invalid(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|_| invalid("private validator original read failed"))?;
        drop(client);
        let original = ValidatorOriginal {
            subject: Arc::new(subject),
            selection: Arc::new(selection),
            context,
            root,
        };
        if let Some(state) = self.read_private_validator_original(&original).await? {
            return Ok(state);
        }
        let result = self.prepare_fresh_private_validator(&original).await;
        match result {
            Ok(state) => Ok(state),
            Err(primary) => match self.read_private_validator_original(&original).await {
                Ok(Some(state)) => Ok(state),
                Ok(None) => Err(primary),
                Err(readback) => Err(invalid(format!(
                    "private validator original unresolved after {primary}; readback: {readback}"
                ))),
            },
        }
    }

    #[cfg(test)]
    async fn prepare_fresh_private_validator(
        &self,
        original: &ValidatorOriginal,
    ) -> Result<brassclaw_turns::private_work::PrivateWorkState, RebornBuildError> {
        let store = PgTurnStateStore::new(self.pool.clone(), original.context.tenant_id.as_str());
        let preparation = self.capture_task_preparation().await?;
        #[cfg(test)]
        {
            let gate = self.capture_gate.lock().await.take();
            if let Some((captured, release)) = gate {
                captured.notify_one();
                release.notified().await;
            }
        }
        let prepared = crate::pg_review_admission_source::PreparedValidatorReview::prepare(
            original.subject.clone(),
            original.selection.clone(),
            self.worker_image.clone(),
            preparation,
        )
        .await?;
        let capture = prepared.input_capture();
        let request = PrivateWorkRequest::V1 {
            work_id: original.subject.submission_id(),
            run_id: TurnRunId::from_uuid(original.subject.submission_id()),
            original: PrivateOriginalRef::ValidatorStructural {
                submission_id: original.subject.submission_id(),
                subject_checksum: decode_checksum(prepared.subject().subject_checksum())?,
            },
            context: original.context.clone(),
            original_preparation_checksum: Some(decode_checksum(capture.preparation().checksum())?),
            original_workflow_checksum: Some(decode_checksum(capture.validator_checksum())?),
        };
        // All original bytes and the Ready seal commit together. No interrupted
        // producer leaves only a hash whose original limits cannot be restored.
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| invalid("private validator retention unavailable"))?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| invalid("private validator retention unavailable"))?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!(
                "brassclaw-validator-original:{}",
                original.subject.submission_id()
            )],
        )
        .await
        .map_err(|_| invalid("private validator original serialization unavailable"))?;
        if store
            .get_private_work_state_in_transaction(&tx, original.subject.submission_id())
            .await
            .map_err(|_| invalid("private validator original unavailable"))?
            .is_some()
        {
            // Do not compare a second offered ACK capture with immutable bytes.
            // Leave this transaction before the one strict original readback.
            return Err(invalid(
                "private validator original won concurrently; readback required",
            ));
        }
        let request_checksum = store
            .submit_private_work_in_transaction(
                &tx,
                &request,
                PrivatePreparationReason::PreparationPending,
                self,
            )
            .await
            .map_err(|_| invalid("private validator original retention failed"))?;
        let state = store
            .get_private_work_state_in_transaction(&tx, original.subject.submission_id())
            .await
            .map_err(|_| invalid("private validator original unavailable"))?
            .ok_or_else(|| invalid("private validator original missing"))?;
        if state.status == PrivateWorkStatus::AwaitingPreparation {
            let definition = PrivateWorkDefinition::capture(DefinitionCapture {
                request: &request,
                request_checksum,
                preparation: capture.preparation().clone(),
                worker_checksum: self.worker_image.checksum(),
                root: &original.root,
                workflow_checksum: decode_checksum(capture.validator_checksum())?,
                input_checksum: decode_checksum(capture.input_checksum())?,
                domain: DomainCapture::ValidatorStructural {
                    capture_bytes: capture.bytes().to_owned(),
                    capture_checksum: decode_checksum(capture.checksum())?,
                },
            })?;
            self.retain_private_definition(&tx, &definition, &store, &state)
                .await?;
        }
        tx.commit()
            .await
            .map_err(|_| invalid("private validator preparation commit unknown"))?;
        drop(client);
        if let Some(wake) = self.private_work_wake.get() {
            wake.wake();
        }
        store
            .get_private_work_state(original.subject.submission_id())
            .await
            .map_err(|_| invalid("private validator published original unavailable"))?
            .ok_or_else(|| invalid("private validator published original missing"))
    }

    #[cfg(test)]
    async fn read_private_validator_original(
        &self,
        original: &ValidatorOriginal,
    ) -> Result<Option<brassclaw_turns::private_work::PrivateWorkState>, RebornBuildError> {
        use brassclaw_turns::pg_turn_state_store::PgPrivateOriginalVerifier;
        let unresolved = || invalid("private validator complete original is unresolved");
        let mut client = self.pool.get().await.map_err(|_| unresolved())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| unresolved())?;
        let Some(state) = PgTurnStateStore::read_private_work_state_in_transaction(
            &tx,
            original.context.tenant_id.as_str(),
            original.subject.submission_id(),
        )
        .await
        .map_err(|_| unresolved())?
        else {
            tx.commit().await.map_err(|_| unresolved())?;
            return Ok(None);
        };
        if state.request.work_id() != original.subject.submission_id()
            || state.request.run_id() != TurnRunId::from_uuid(original.subject.submission_id())
            || state.request.context() != &original.context
            || !matches!(state.request.original(),PrivateOriginalRef::ValidatorStructural { submission_id, subject_checksum }
                if *submission_id==original.subject.submission_id() && *subject_checksum==decode_checksum(original.subject.subject_checksum())?)
        {
            return Err(unresolved());
        }
        self.verify_original(&tx, &state.request)
            .await
            .map_err(|_| unresolved())?;
        let row = tx.query_opt("SELECT definition_bytes,checksum FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&state.request.work_id()]).await.map_err(|_| unresolved())?.ok_or_else(unresolved)?;
        let definition = PrivateWorkDefinition::restore(
            &state.request,
            state.request_checksum,
            row.get(0),
            row.get::<_, Vec<u8>>(1)
                .try_into()
                .map_err(|_| unresolved())?,
        )?;
        if definition.root().reference() != original.root.reference()
            || definition.worker_checksum() != self.worker_image.checksum()
            || state.request.original_preparation_checksum()
                != Some(decode_checksum(definition.preparation().checksum())?)
            || state.request.original_workflow_checksum() != Some(definition.workflow_checksum())
            || definition.workflow_checksum() != checksum(original.selection.validator_bytes())
        {
            return Err(unresolved());
        }
        // Only coherent metadata disposition is returned. No SQL value creates
        // a live completion, reopens dispatch, renews a lease or grants approval.
        let coherent: bool = tx.query_one("SELECT COALESCE(w.private_definition_checksum=$3 AND w.version=$4 AND
            CASE WHEN w.status IN ('ready','cancelled') THEN
                w.private_claim_generation=0 AND w.private_claim_attempts=0 AND w.private_runner_id IS NULL
                AND w.private_claim_checksum IS NULL AND w.private_admission_run IS NULL AND w.private_lease_deadline IS NULL
                AND a.run_id IS NULL AND t.work_id IS NULL AND w.private_terminal_checksum IS NULL
                AND ((w.status='ready' AND w.private_cancel_revision=0) OR (w.status='cancelled' AND w.private_cancel_revision>0))
            WHEN w.status IN ('claimed','reconciliation_required','settled') THEN
                w.private_claim_generation=1 AND w.private_claim_attempts=1 AND w.private_runner_id IS NOT NULL
                AND octet_length(w.private_claim_checksum)=32 AND w.private_admission_run=w.execution_run_id
                AND a.origin_kind='private_work' AND a.private_work_id=w.work_id AND a.run_id=w.execution_run_id
                AND a.runner_id=w.private_runner_id AND a.claim_checksum=w.private_claim_checksum
                AND a.private_claim_generation=w.private_claim_generation AND a.private_request_checksum=w.private_request_checksum
                AND a.private_execution_context=w.private_request_bytes::jsonb->'context' AND a.native_actor=$5
                AND a.turn_id IS NULL AND a.scope IS NULL AND a.accepted_message_ref IS NULL
                AND CASE WHEN w.status='settled' THEN a.phase='settled' AND t.run_id=w.execution_run_id
                    AND t.checksum=w.private_terminal_checksum AND t.checksum=sha256(convert_to(t.terminal_bytes,'UTF8'))
                ELSE a.phase IN ('reserved','started') AND t.work_id IS NULL AND w.private_terminal_checksum IS NULL END
            ELSE false END,false)
            FROM brassclaw_turns w LEFT JOIN brassclaw_monty_task_admissions a ON a.run_id=w.private_admission_run
            LEFT JOIN brassclaw_monty_private_terminals t ON t.work_id=w.work_id
            WHERE w.record_kind='private_work' AND w.tenant_id=$1 AND w.work_id=$2",
            &[&original.context.tenant_id.as_str(),&state.request.work_id(),&&definition.checksum()[..],
                &i64::try_from(state.version).map_err(|_| unresolved())?,&serde_json::to_value(&original.context.actor).map_err(|_| unresolved())?])
            .await.map_err(|_| unresolved())?.get(0);
        if !coherent {
            return Err(unresolved());
        }
        tx.commit().await.map_err(|_| unresolved())?;
        drop(client);
        let DomainCapture::ValidatorStructural {
            capture_bytes,
            capture_checksum,
        } = definition.domain()
        else {
            return Err(unresolved());
        };
        // The existing A03 strict codec/inspector verifies original raw subject,
        // selection, input, preparation and actual worker. No PG transaction or
        // connection is held across native/parser execution.
        let prepared =
            crate::pg_review_admission_source::PreparedValidatorReview::restore_original(
                original.subject.clone(),
                original.selection.clone(),
                self.worker_image.clone(),
                definition.preparation().clone(),
                capture_bytes,
                &hex::encode(capture_checksum),
            )
            .await?;
        if prepared.input_capture().input_checksum() != hex::encode(definition.input_checksum())
            || prepared.input_capture().validator_checksum()
                != hex::encode(definition.workflow_checksum())
        {
            return Err(unresolved());
        }
        Ok(Some(state))
    }

    pub(crate) async fn read_private_prefix_original(
        &self,
        definition: &crate::prefix_job_definitions::Definition,
    ) -> Result<(), RebornBuildError> {
        let mut client = crate::bootstrap_evidence::io::EvidenceClient::acquire(&self.pool)
            .await
            .map_err(|_| invalid("private prefix original unavailable"))?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| invalid("private prefix original unavailable"))?;
        self.read_private_prefix_original_in_transaction(&tx, definition)
            .await?;
        tx.commit()
            .await
            .map_err(|_| invalid("private prefix original readback unknown"))?;
        client.recycle_after_commit();
        Ok(())
    }

    pub(crate) async fn read_private_prefix_original_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        definition: &crate::prefix_job_definitions::Definition,
    ) -> Result<(), RebornBuildError> {
        use brassclaw_turns::pg_turn_state_store::PgPrivateOriginalVerifier;
        let unresolved =
            || invalid("private prefix original Work and Ready seal unresolved; do not replay");
        let request = Self::private_prefix_request(definition)?;
        let state = PgTurnStateStore::read_private_work_state_in_transaction(
            tx,
            request.context().tenant_id.as_str(),
            definition.id,
        )
        .await
        .map_err(|_| unresolved())?
        .ok_or_else(unresolved)?;
        if state.request != request || state.status == PrivateWorkStatus::AwaitingPreparation {
            return Err(unresolved());
        }
        self.verify_original(tx, &state.request)
            .await
            .map_err(|_| unresolved())?;
        let row = tx
            .query_opt(
                "SELECT p.definition_bytes,p.checksum FROM brassclaw_turns w
            JOIN brassclaw_monty_private_preparations p ON p.work_id=w.work_id
                AND p.checksum=w.private_definition_checksum
            WHERE w.record_kind='private_work' AND w.tenant_id=$1 AND w.work_id=$2",
                &[&request.context().tenant_id.as_str(), &definition.id],
            )
            .await
            .map_err(|_| unresolved())?
            .ok_or_else(unresolved)?;
        let captured = PrivateWorkDefinition::restore(
            &request,
            state.request_checksum,
            row.get(0),
            row.get::<_, Vec<u8>>(1)
                .try_into()
                .map_err(|_| unresolved())?,
        )?;
        let crate::prefix_job_definitions::Origin::Private { preparation, .. } = &definition.origin
        else {
            return Err(unresolved());
        };
        if captured.preparation().bytes() != preparation.bytes()
            || captured.workflow_checksum() != checksum(&definition.program_bytes)
            || captured.input_checksum() != checksum(&definition.input_bytes)
            || !matches!(captured.domain(), DomainCapture::PrefixSourceOnly { job_id } if *job_id == definition.id)
        {
            return Err(unresolved());
        }
        // Historical readback checks the retained root, never today's startup
        // root, worker image, compiler selection or effective limits.
        PgComponentRevisionStore::read_revision_in_transaction(tx, captured.root().reference())
            .await
            .map_err(|_| unresolved())?;
        let coherent: bool = tx
            .query_one(
                "SELECT COALESCE(w.private_request_checksum=$3 AND
            CASE WHEN w.status IN ('ready','cancelled') THEN
                w.private_claim_generation=0 AND w.private_claim_attempts=0
                AND a.run_id IS NULL AND t.work_id IS NULL AND w.private_terminal_checksum IS NULL
            WHEN w.status IN ('claimed','reconciliation_required','settled') THEN
                w.private_claim_generation=1 AND w.private_claim_attempts=1
                AND a.origin_kind='private_work' AND a.private_work_id=w.work_id
                AND a.run_id=w.execution_run_id AND a.runner_id=w.private_runner_id
                AND a.claim_checksum=w.private_claim_checksum
                AND a.private_claim_generation=w.private_claim_generation
                AND a.private_request_checksum=w.private_request_checksum
                AND a.private_execution_context=w.private_request_bytes::jsonb->'context'
                AND a.native_actor=a.private_execution_context->'actor'
                AND a.turn_id IS NULL AND a.scope IS NULL AND a.accepted_message_ref IS NULL
                AND CASE WHEN w.status='settled' THEN a.phase='settled'
                    AND t.run_id=w.execution_run_id AND t.checksum=w.private_terminal_checksum
                    AND t.checksum=sha256(convert_to(t.terminal_bytes,'UTF8'))
                ELSE a.phase IN ('reserved','started') AND t.work_id IS NULL
                    AND w.private_terminal_checksum IS NULL END
            ELSE false END,false)
            FROM brassclaw_turns w
            LEFT JOIN brassclaw_monty_task_admissions a ON a.run_id=w.private_admission_run
            LEFT JOIN brassclaw_monty_private_terminals t ON t.work_id=w.work_id
            WHERE w.record_kind='private_work' AND w.tenant_id=$1 AND w.work_id=$2",
                &[
                    &request.context().tenant_id.as_str(),
                    &definition.id,
                    &&state.request_checksum[..],
                ],
            )
            .await
            .map_err(|_| unresolved())?
            .get(0);
        if !coherent {
            return Err(unresolved());
        }
        Ok(())
    }

    fn private_prefix_request(
        definition: &crate::prefix_job_definitions::Definition,
    ) -> Result<PrivateWorkRequest, RebornBuildError> {
        let crate::prefix_job_definitions::Origin::Private {
            context,
            preparation,
        } = &definition.origin
        else {
            return Err(invalid("private prefix origin required"));
        };
        Ok(PrivateWorkRequest::V1 {
            work_id: definition.id,
            run_id: TurnRunId::from_uuid(definition.id),
            original: PrivateOriginalRef::PrefixSourceOnly {
                job_id: definition.id,
                definition_checksum: definition.original_checksum(),
            },
            context: context.clone(),
            original_preparation_checksum: Some(decode_checksum(preparation.checksum())?),
            original_workflow_checksum: Some(checksum(&definition.program_bytes)),
        })
    }

    // Historical protocol fixtures deliberately create metadata before Work so
    // native claim/ACK/cancellation boundaries can be exercised independently.
    #[cfg(test)]
    pub(crate) async fn prepare_private_prefix_for_test(
        &self,
        definition: crate::prefix_job_definitions::Definition,
    ) -> Result<(), RebornBuildError> {
        let mut client = crate::bootstrap_evidence::io::EvidenceClient::acquire(&self.pool)
            .await
            .map_err(|_| invalid("private prefix preparation unavailable"))?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| invalid("private prefix preparation unavailable"))?;
        self.prepare_private_prefix_definition_in_transaction(&tx, &definition)
            .await?;
        tx.commit()
            .await
            .map_err(|_| invalid("private prefix preparation commit unknown"))?;
        client.recycle_after_commit();
        Ok(())
    }

    pub(crate) async fn prepare_private_prefix_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        reserved: &crate::prefix_job_definitions::ReservedPrivateDefinition,
    ) -> Result<(), RebornBuildError> {
        if !reserved.newly_retained() {
            return Err(invalid(
                "only a newly retained prefix original may prepare Work",
            ));
        }
        self.prepare_private_prefix_definition_in_transaction(tx, reserved.definition())
            .await
    }

    async fn prepare_private_prefix_definition_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        definition: &crate::prefix_job_definitions::Definition,
    ) -> Result<(), RebornBuildError> {
        let crate::prefix_job_definitions::Origin::Private {
            context,
            preparation,
        } = &definition.origin
        else {
            return Err(invalid("private prefix origin required"));
        };
        let request = Self::private_prefix_request(definition)?;
        let store = PgTurnStateStore::new(self.pool.clone(), context.tenant_id.as_str());
        let request_checksum = store
            .submit_private_work_in_transaction(
                tx,
                &request,
                PrivatePreparationReason::PreparationPending,
                self,
            )
            .await
            .map_err(|_| invalid("private prefix original retention failed"))?;
        // submit holds the real Work row lock through definition retention and
        // Ready CAS. An identical concurrent producer observes its committed
        // winner before inspecting/preparing again; there is no second seal.
        let state = store
            .get_private_work_state_in_transaction(tx, definition.id)
            .await
            .map_err(|_| invalid("private prefix original unavailable"))?
            .ok_or_else(|| invalid("private prefix original unavailable"))?;
        if state.status != PrivateWorkStatus::AwaitingPreparation {
            return Ok(());
        }
        let program = self
            ._compiler_program
            .as_ref()
            .ok_or_else(|| invalid("source compiler qualification unavailable"))?;
        if program
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|error| invalid(error.to_string()))?
            .exact_bytes()
            != definition.program_bytes
        {
            return Err(invalid("source compiler original differs"));
        }
        let approval = self
            .compiler_approval
            .as_ref()
            .ok_or_else(|| invalid("source compiler qualification unavailable"))?;
        approval.verify_in_transaction(tx).await?;
        approval.require_selected_program(program)?;
        let root = PgComponentRevisionStore::read_revision_in_transaction(tx, self.root_reference)
            .await
            .map_err(|error| invalid(error.to_string()))?;
        let captured = PrivateWorkDefinition::capture(DefinitionCapture {
            request: &request,
            request_checksum,
            preparation: preparation.as_ref().clone(),
            worker_checksum: self.worker_image.checksum(),
            root: &root,
            workflow_checksum: checksum(&definition.program_bytes),
            input_checksum: checksum(&definition.input_bytes),
            domain: DomainCapture::PrefixSourceOnly {
                job_id: definition.id,
            },
        })?;
        self.retain_private_definition(tx, &captured, &store, &state)
            .await
    }

    pub(crate) async fn retain_private_definition(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        definition: &PrivateWorkDefinition,
        store: &PgTurnStateStore,
        state: &brassclaw_turns::private_work::PrivateWorkState,
    ) -> Result<(), RebornBuildError> {
        tx.execute(
            "INSERT INTO brassclaw_monty_private_preparations(work_id,definition_bytes,checksum)
            VALUES($1,$2,$3) ON CONFLICT(work_id) DO NOTHING",
            &[
                &state.request.work_id(),
                &definition.bytes(),
                &&definition.checksum()[..],
            ],
        )
        .await
        .map_err(|_| invalid("private preparation retention failed"))?;
        let row = tx
            .query_one(
                "SELECT definition_bytes,checksum FROM brassclaw_monty_private_preparations
            WHERE work_id=$1",
                &[&state.request.work_id()],
            )
            .await
            .map_err(|_| invalid("private preparation original unavailable"))?;
        if row.get::<_, &str>(0) != definition.bytes()
            || row.get::<_, Vec<u8>>(1).as_slice() != definition.checksum()
        {
            return Err(invalid("private preparation original conflicts"));
        }
        store
            .seal_private_work_in_transaction(
                tx,
                state.request.work_id(),
                i64::try_from(state.version)
                    .map_err(|_| invalid("private preparation version invalid"))?,
                state.request_checksum,
                definition.checksum(),
            )
            .await
            .map_err(|_| invalid("private preparation seal conflicts"))
    }

    pub(super) async fn capture_private_original(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
    ) -> Result<crate::global_task_factory::CapturedPrivateRecipe, AgentLoopDriverError> {
        let claim = host.private_claim().ok_or_else(failed)?;
        let mut client = self.pool.get().await.map_err(|_| failed())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failed())?;
        let row = tx
            .query_opt(
                "SELECT definition_bytes,checksum FROM brassclaw_monty_private_preparations
            WHERE work_id=$1",
                &[&claim.request().work_id()],
            )
            .await
            .map_err(|_| failed())?
            .ok_or_else(failed)?;
        let bytes: String = row.get(0);
        let expected: [u8; 32] = row.get::<_, Vec<u8>>(1).try_into().map_err(|_| failed())?;
        if expected != claim.definition_checksum() {
            return Err(failed());
        }
        let captured = Arc::new(
            PrivateWorkDefinition::restore(
                claim.request(),
                claim.request_checksum(),
                &bytes,
                expected,
            )
            .map_err(|_| failed())?,
        );
        if captured.worker_checksum() != self.worker_image.checksum()
            || captured.root().reference() != self.root_reference
        {
            return Err(failed());
        }
        PgComponentRevisionStore::read_revision_in_transaction(&tx, captured.root().reference())
            .await
            .map_err(|_| failed())?;
        match captured.domain() {
            DomainCapture::PrefixSourceOnly { job_id } => {
                let definition =
                    crate::prefix_job_definitions::Store::get_in_transaction(&tx, *job_id)
                        .await
                        .map_err(|_| failed())?
                        .ok_or_else(failed)?;
                let PrivateOriginalRef::PrefixSourceOnly {
                    definition_checksum,
                    ..
                } = claim.request().original()
                else {
                    return Err(failed());
                };
                let crate::prefix_job_definitions::Origin::Private {
                    preparation,
                    context,
                } = &definition.origin
                else {
                    return Err(failed());
                };
                let program = self._compiler_program.as_ref().ok_or_else(failed)?;
                let approval = self.compiler_approval.as_ref().ok_or_else(failed)?;
                if definition.original_checksum() != *definition_checksum
                    || context != claim.request().context()
                    || definition.operation != "generate"
                    || definition.mode != "source_only"
                    || preparation.bytes() != captured.preparation().bytes()
                    || checksum(&definition.program_bytes) != captured.workflow_checksum()
                    || checksum(&definition.input_bytes) != captured.input_checksum()
                    || program
                        .program()
                        .inputs()
                        .instruction()
                        .retained_selection()
                        .map_err(|_| failed())?
                        .exact_bytes()
                        != definition.program_bytes
                {
                    return Err(failed());
                }
                let (profile, revision) = definition.target.profile().ok_or_else(failed)?;
                if !matches!(profile, "base-prompt" | "prefix-maker" | "sempai")
                    || definition.input["profile"] != profile
                    || definition.input["revision"] != revision.to_string()
                {
                    return Err(failed());
                }
                crate::prefix_job_execution::validate_snapshot(
                    profile,
                    definition.input["snapshot_bytes"]
                        .as_str()
                        .ok_or_else(failed)?,
                    definition.original_input_limits,
                )
                .map_err(|_| failed())?;
                approval
                    .verify_in_transaction(&tx)
                    .await
                    .map_err(|_| failed())?;
                approval
                    .require_selected_program(program)
                    .map_err(|_| failed())?;
                let retained = program.program().clone();
                tx.commit().await.map_err(|_| failed())?;
                drop(client);
                let inspected = Arc::new(
                    InspectedRetainedProgram::inspect_with_limits(
                        retained,
                        &self.worker_image.path(),
                        captured.preparation().limits(),
                    )
                    .await
                    .map_err(|_| failed())?,
                );
                inspected
                    .program()
                    .inputs()
                    .bind_task_inputs_with_limits(
                        &definition.input,
                        definition.original_input_limits,
                    )
                    .map_err(|_| failed())?;
                let RetainedProgram::Tools(program) = inspected.program() else {
                    return Err(failed());
                };
                let program = program.clone();
                let pool = self.pool.clone();
                let kernel = self.kernel.clone();
                let source = Arc::new(crate::prefix_job_execution::Invocation {
                    reference: format!("prefix-job:{}", definition.id),
                    definition,
                    inspected,
                    receipt_contract: crate::prefix_job_execution::ReceiptContract::Generation,
                    tools: Default::default(),
                    bind_tools: Box::new(move || {
                        bootstrap_compiler_tools(
                            pool.clone(),
                            kernel.clone(),
                            host.clone(),
                            admission.clone(),
                            program.clone(),
                            false,
                        )
                    }),
                    pool: self.pool.clone(),
                });
                Ok(crate::global_task_factory::CapturedPrivateRecipe {
                    definition: captured,
                    source,
                })
            }
            DomainCapture::ValidatorStructural {
                capture_bytes,
                capture_checksum,
            } => {
                use brassclaw_skills::component_revision::REVISION_LIMITS;
                use brassclaw_skills::review_submission_store::{
                    PgReviewSubmissionStore, selection,
                };
                let PrivateOriginalRef::ValidatorStructural {
                    submission_id,
                    subject_checksum,
                } = claim.request().original()
                else {
                    return Err(failed());
                };
                let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
                    &tx,
                    *submission_id,
                    REVISION_LIMITS,
                )
                .await
                .map_err(|_| failed())?
                .ok_or_else(failed)?;
                let selection = selection::read_preparation_in_transaction(
                    &tx,
                    *submission_id,
                    REVISION_LIMITS,
                )
                .await
                .map_err(|_| failed())?
                .ok_or_else(failed)?;
                if decode_checksum(subject.subject_checksum()).map_err(|_| failed())?
                    != *subject_checksum
                {
                    return Err(failed());
                }
                self.require_source_selection(&tx, &selection)
                    .await
                    .map_err(|_| failed())?;
                let source_profile = selection.source_qualification().is_some();
                tx.commit().await.map_err(|_| failed())?;
                drop(client);
                let capture_checksum = hex::encode(capture_checksum);
                let prepared =
                    crate::pg_review_admission_source::PreparedValidatorReview::restore_original(
                        Arc::new(subject),
                        Arc::new(selection),
                        self.worker_image.clone(),
                        captured.preparation().clone(),
                        capture_bytes,
                        &capture_checksum,
                    )
                    .await
                    .map_err(|_| failed())?;
                if decode_checksum(prepared.input_capture().validator_checksum())
                    .map_err(|_| failed())?
                    != captured.workflow_checksum()
                    || decode_checksum(prepared.input_capture().input_checksum())
                        .map_err(|_| failed())?
                        != captured.input_checksum()
                {
                    return Err(failed());
                }
                let source: Arc<dyn crate::global_recipe_ports::InternalRecipeSource> =
                    if source_profile {
                        Arc::new(
                            crate::validator_inventory_review::InventoryReviewSource::from_qualified(
                                prepared,
                                self.pure_source_review.parser().map_err(|_| failed())?,
                                self,
                                host,
                                admission,
                            )
                            .map_err(|_| failed())?,
                        )
                    } else {
                        Arc::new(
                            crate::pg_review_admission_source::ReviewAdmissionSource::from_prepared(
                                prepared,
                                self.pool.clone(),
                            )
                            .map_err(|_| failed())?,
                        )
                    };
                Ok(crate::global_task_factory::CapturedPrivateRecipe {
                    definition: captured,
                    source,
                })
            }
        }
    }
}
