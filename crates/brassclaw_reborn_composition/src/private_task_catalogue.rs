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
        let store = PgTurnStateStore::new(self.pool.clone(), context.tenant_id.as_str());
        // An original already published by this owner must never capture today's
        // settings again, including after an unknown COMMIT acknowledgement.
        if let Some(state) = store
            .get_private_work_state(submission_id)
            .await
            .map_err(|_| invalid("private validator original unavailable"))?
        {
            if !matches!(state.request.original(), PrivateOriginalRef::ValidatorStructural { submission_id: original, .. } if *original == submission_id)
            {
                return Err(invalid("private validator original conflicts"));
            }
            return Ok(state);
        }
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
        let prepared = crate::pg_review_admission_source::PreparedValidatorReview::prepare(
            Arc::new(subject),
            Arc::new(selection),
            self.worker_image.clone(),
            self.capture_task_preparation().await?,
        )
        .await?;
        let capture = prepared.input_capture();
        let request = PrivateWorkRequest::V1 {
            work_id: submission_id,
            run_id: TurnRunId::from_uuid(submission_id),
            original: PrivateOriginalRef::ValidatorStructural {
                submission_id,
                subject_checksum: decode_checksum(prepared.subject().subject_checksum())?,
            },
            context,
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
            .get_private_work_state_in_transaction(&tx, submission_id)
            .await
            .map_err(|_| invalid("private validator original unavailable"))?
            .ok_or_else(|| invalid("private validator original missing"))?;
        if state.status == PrivateWorkStatus::AwaitingPreparation {
            let definition = PrivateWorkDefinition::capture(DefinitionCapture {
                request: &request,
                request_checksum,
                preparation: capture.preparation().clone(),
                worker_checksum: self.worker_image.checksum(),
                root: &root,
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
            .get_private_work_state(submission_id)
            .await
            .map_err(|_| invalid("private validator published original unavailable"))?
            .ok_or_else(|| invalid("private validator published original missing"))
    }

    pub(crate) async fn prepare_private_prefix(
        &self,
        definition: crate::prefix_job_definitions::Definition,
    ) -> Result<(), RebornBuildError> {
        let crate::prefix_job_definitions::Origin::Private {
            context,
            preparation,
        } = &definition.origin
        else {
            return Err(invalid("private prefix origin required"));
        };
        let request = PrivateWorkRequest::V1 {
            work_id: definition.id,
            run_id: TurnRunId::from_uuid(definition.id),
            original: PrivateOriginalRef::PrefixSourceOnly {
                job_id: definition.id,
                definition_checksum: definition.original_checksum(),
            },
            context: context.clone(),
            original_preparation_checksum: Some(decode_checksum(preparation.checksum())?),
            original_workflow_checksum: Some(checksum(&definition.program_bytes)),
        };
        let store = PgTurnStateStore::new(self.pool.clone(), context.tenant_id.as_str());
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| invalid("private prefix preparation unavailable"))?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| invalid("private prefix preparation unavailable"))?;
        let request_checksum = store
            .submit_private_work_in_transaction(
                &tx,
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
            .get_private_work_state_in_transaction(&tx, definition.id)
            .await
            .map_err(|_| invalid("private prefix original unavailable"))?
            .ok_or_else(|| invalid("private prefix original unavailable"))?;
        if state.status != PrivateWorkStatus::AwaitingPreparation {
            return tx
                .commit()
                .await
                .map_err(|_| invalid("private prefix original read commit unknown"));
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
        approval.verify_in_transaction(&tx).await?;
        approval.require_selected_program(program)?;
        let root = PgComponentRevisionStore::read_revision_in_transaction(&tx, self.root_reference)
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
        self.retain_private_definition(&tx, &captured, &store, &state)
            .await?;
        tx.commit()
            .await
            .map_err(|_| invalid("private prefix preparation commit unknown"))
    }

    async fn retain_private_definition(
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
                let source = Arc::new(
                    crate::pg_review_admission_source::ReviewAdmissionSource::from_prepared(
                        prepared,
                        self.pool.clone(),
                    )
                    .map_err(|_| failed())?,
                );
                Ok(crate::global_task_factory::CapturedPrivateRecipe {
                    definition: captured,
                    source,
                })
            }
        }
    }
}
