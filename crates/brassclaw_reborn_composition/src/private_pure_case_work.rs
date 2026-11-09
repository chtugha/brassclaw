//! Authored original case -> existing private Work/Ready/admission owner.
//! The API prepares exactly one case; it does not drive a case list or execute
//! candidate Python. Actual execution stays on the already running global VM.
use super::*;
use crate::{
    pg_transaction_client::TransactionClient,
    private_work_definition::{
        DefinitionCapture, DomainCapture, PrivateWorkDefinition, checksum, decode_checksum,
    },
    pure_case_original::CheckedOriginalPureCase,
};
use brassclaw_engine::executor::retained_source::InspectedPureSelection;
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use brassclaw_turns::{
    PgTurnStateStore, TurnRunId,
    private_work::{
        PrivateExecutionContext, PrivateOriginalRef, PrivatePreparationReason, PrivateWorkRequest,
        PrivateWorkState, PrivateWorkStatus,
    },
};
use tokio_postgres::{IsolationLevel, Transaction};

impl InstalledMontyCatalogue {
    async fn pure_case_context(
        &self,
        tx: &Transaction<'_>,
        submission: Uuid,
    ) -> Result<PrivateExecutionContext, RebornBuildError> {
        let mut context = self
            .private_work_context
            .get()
            .cloned()
            .ok_or_else(|| invalid("original Pure case instance context unavailable"))?;
        let actor: String = tx
            .query_one(
                "SELECT actor FROM reborn_component_review_submissions WHERE submission_id=$1",
                &[&submission],
            )
            .await
            .map_err(invalid)?
            .get(0);
        context.actor = Some(brassclaw_turns::TurnActor::new(
            brassclaw_host_api::UserId::new(actor).map_err(invalid)?,
        ));
        Ok(context)
    }

    pub(crate) async fn prepare_private_pure_case(
        &self,
        case_id: Uuid,
        submission: Uuid,
        export: &str,
        inputs: &Value,
    ) -> Result<PrivateWorkState, RebornBuildError> {
        if case_id.is_nil() || case_id == submission {
            return Err(invalid("distinct original Pure case ID required"));
        }
        let timeout = self
            .resource_store_settings
            .snapshot()
            .map_err(invalid)?
            .validate()
            .map_err(invalid)?
            .timeout;
        // Same-ID reads precede any current-settings capture or reinspection.
        if let Some(state) = tokio::time::timeout(
            timeout,
            self.read_original_case_state(case_id, submission, export, inputs),
        )
        .await
        .map_err(|_| invalid("original Pure case read deadline elapsed"))??
        {
            return Ok(state);
        }
        let qualified = self
            .case_harness
            .as_ref()
            .ok_or_else(|| invalid("original Pure case capability is unready"))?;
        let preparation = self.capture_task_preparation().await?;
        let bounds = preparation.limits().bounds;
        let limits = brassclaw_skills::value_contract::ContractLimits {
            max_depth: bounds.max_value_depth,
            max_nodes: bounds.max_value_nodes,
            max_bytes: bounds.max_value_bytes,
        };
        brassclaw_skills::value_contract::validate_data_bounds(inputs, limits).map_err(invalid)?;
        let (context, original) = tokio::time::timeout(timeout, async {
            let mut client =
                TransactionClient::from_client(self.pool.get().await.map_err(invalid)?);
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            let context = self.pure_case_context(&tx, submission).await?;
            let source = self
                .pure_source_review
                .read_positive_original(&tx, submission, &context)
                .await?
                .ok_or_else(|| {
                    invalid("positive original Source prerequisite is not acknowledged")
                })?;
            let original = Arc::new(
                CheckedOriginalPureCase::capture(
                    &tx,
                    &source,
                    &context,
                    case_id,
                    export,
                    inputs.clone(),
                )
                .await?,
            );
            qualified.require(&tx).await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            drop(client);
            Ok::<_, RebornBuildError>((context, original))
        })
        .await
        .map_err(|_| invalid("original Pure Source prerequisite read deadline elapsed"))??;
        if original.worker_checksum() != self.worker_image.checksum() {
            return Err(invalid(
                "original Pure case requires its retained Source worker",
            ));
        }
        let worker = crate::pg_review_admission_source::verified_worker(
            &self.worker_image,
            preparation.limits().max_native_artifact_bytes,
        )
        .await?;
        let inspected = InspectedPureSelection::inspect(
            original.snapshot_ref(),
            original.candidate().uuid,
            &worker.path(),
            preparation.limits(),
        )
        .await
        .map_err(invalid)?;
        // Qualify every export/closure above, then bind exactly this declared
        // export and typed original input. Loading/invocation occurs later.
        inspected
            .invocation(export, inputs, limits)
            .map_err(invalid)?;
        let write = tokio::time::timeout(timeout, async {
            let mut client =
                TransactionClient::from_client(self.pool.get().await.map_err(invalid)?);
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .map_err(invalid)?;
            let source = self
                .pure_source_review
                .read_positive_original(&tx, submission, &context)
                .await?
                .ok_or_else(|| invalid("positive original Source prerequisite disappeared"))?;
            let current = CheckedOriginalPureCase::capture(
                &tx,
                &source,
                &context,
                case_id,
                export,
                inputs.clone(),
            )
            .await?;
            if current.bytes() != original.bytes() {
                return Err(invalid("original Pure case changed during preparation"));
            }
            qualified.require(&tx).await?;
            current.retain(&tx).await?;
            let selection = qualified
                .program()
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(invalid)?;
            let workflow = checksum(selection.exact_bytes());
            let request = PrivateWorkRequest::V1 {
                work_id: case_id,
                run_id: TurnRunId::from_uuid(case_id),
                original: PrivateOriginalRef::OriginalPureCaseV1 {
                    case_id,
                    case_checksum: current.checksum(),
                },
                context: context.clone(),
                original_preparation_checksum: Some(decode_checksum(preparation.checksum())?),
                original_workflow_checksum: Some(workflow),
            };
            let store = PgTurnStateStore::new(self.pool.clone(), context.tenant_id.as_str());
            let verifier = CaseWriteVerifier {
                tx: &tx,
                case: &current,
            };
            let request_checksum = store
                .submit_private_work_in_transaction(
                    &tx,
                    &request,
                    PrivatePreparationReason::PreparationPending,
                    &verifier,
                )
                .await
                .map_err(invalid)?;
            let state = store
                .get_private_work_state_in_transaction(&tx, case_id)
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("original Pure Work missing after insertion"))?;
            let root =
                PgComponentRevisionStore::read_revision_in_transaction(&tx, self.root_reference)
                    .await
                    .map_err(invalid)?;
            let definition = PrivateWorkDefinition::capture(DefinitionCapture {
                request: &request,
                request_checksum,
                preparation: preparation.clone(),
                worker_checksum: current.worker_checksum(),
                root: &root,
                workflow_checksum: workflow,
                input_checksum: checksum("{}"),
                domain: DomainCapture::OriginalPureCaseV1 {
                    case_id,
                    case_checksum: current.checksum(),
                },
            })?;
            self.retain_private_definition(&tx, &definition, &store, &state)
                .await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(())
        })
        .await
        .map_err(|_| invalid("original Pure case preparation write deadline elapsed"))
        .and_then(|r| r);
        let read = tokio::time::timeout(
            timeout,
            self.read_original_case_state(case_id, submission, export, inputs),
        )
        .await
        .map_err(|_| invalid("original Pure case preparation readback deadline elapsed"))?;
        let state = match read {
            Ok(Some(state)) => state,
            Ok(None) => {
                write?;
                return Err(invalid(
                    "original Pure case preparation remains unacknowledged",
                ));
            }
            Err(error) => {
                write?;
                return Err(error);
            }
        };
        if let Some(wake) = self.private_work_wake.get() {
            wake.wake();
        }
        Ok(state)
    }

    async fn read_original_case_state(
        &self,
        case_id: Uuid,
        submission: Uuid,
        export: &str,
        inputs: &Value,
    ) -> Result<Option<PrivateWorkState>, RebornBuildError> {
        let mut client = TransactionClient::from_client(self.pool.get().await.map_err(invalid)?);
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let context = self.pure_case_context(&tx, submission).await?;
        let Some(case) =
            CheckedOriginalPureCase::read(&tx, &self.pure_source_review, case_id, &context).await?
        else {
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            return Ok(None);
        };
        if case.submission_id() != submission || case.export() != export || case.inputs() != inputs
        {
            return Err(invalid(
                "same original Pure case ID has different input or export",
            ));
        }
        let state = PgTurnStateStore::read_private_work_state_in_transaction(
            &tx,
            context.tenant_id.as_str(),
            case_id,
        )
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("original Pure case Work is missing"))?;
        if state.status == PrivateWorkStatus::AwaitingPreparation
            || state.request.context() != &context
            || !matches!(state.request.original(),PrivateOriginalRef::OriginalPureCaseV1 {case_id:id,case_checksum}
                if *id==case_id && *case_checksum==case.checksum())
        {
            return Err(invalid("original Pure case Ready seal differs"));
        }
        let row=tx.query_one("SELECT definition_bytes,checksum FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&case_id]).await.map_err(invalid)?;
        let definition = PrivateWorkDefinition::restore(
            &state.request,
            state.request_checksum,
            row.get(0),
            row.get::<_, Vec<u8>>(1)
                .try_into()
                .map_err(|_| invalid("original Pure definition checksum invalid"))?,
        )?;
        if state.request.original_preparation_checksum().is_none()
            || state.request.original_workflow_checksum().is_none()
            || definition.worker_checksum() != case.worker_checksum()
            || definition.input_checksum() != checksum("{}")
        {
            return Err(invalid("original Pure case captured definition differs"));
        }
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(Some(state))
    }

    pub(super) async fn read_private_case_for_dispatch(
        &self,
        tx: &Transaction<'_>,
        claim: &brassclaw_turns::private_work::ClaimedPrivateWork,
        definition: &PrivateWorkDefinition,
    ) -> Result<Arc<CheckedOriginalPureCase>, RebornBuildError> {
        let PrivateOriginalRef::OriginalPureCaseV1 {
            case_id,
            case_checksum,
        } = claim.request().original()
        else {
            return Err(invalid("original Pure dispatch purpose differs"));
        };
        let case = CheckedOriginalPureCase::read(
            tx,
            &self.pure_source_review,
            *case_id,
            claim.request().context(),
        )
        .await?
        .ok_or_else(|| invalid("original Pure dispatch case missing"))?;
        let qualified = self
            .case_harness
            .as_ref()
            .ok_or_else(|| invalid("original Pure case capability is unready"))?;
        qualified.require(tx).await?;
        let program = qualified.program();
        if case.checksum() != *case_checksum
            || case.id() != claim.request().work_id()
            || definition.worker_checksum() != case.worker_checksum()
            || definition.input_checksum() != checksum("{}")
            || definition.workflow_checksum()
                != checksum(
                    program
                        .inputs()
                        .instruction()
                        .retained_selection()
                        .map_err(invalid)?
                        .exact_bytes(),
                )
        {
            return Err(invalid("original Pure dispatch retained selection differs"));
        }
        Ok(Arc::new(case))
    }
}

/// Reuses the just-checked original in the same caller-owned transaction. It
/// cannot become a global cache, read another snapshot or approve a case.
struct CaseWriteVerifier<'checked, 'connection> {
    tx: &'checked Transaction<'connection>,
    case: &'checked CheckedOriginalPureCase,
}
#[async_trait]
impl brassclaw_turns::pg_turn_state_store::PgPrivateOriginalVerifier for CaseWriteVerifier<'_, '_> {
    async fn verify_original(
        &self,
        tx: &Transaction<'_>,
        request: &PrivateWorkRequest,
    ) -> Result<(), brassclaw_turns::TurnError> {
        let error = || brassclaw_turns::TurnError::Unavailable {
            reason: "original Pure case transaction binding differs".into(),
        };
        if std::ptr::from_ref(self.tx).cast::<()>() != std::ptr::from_ref(tx).cast::<()>()
            || request.context() != self.case.context()
            || request.work_id() != self.case.id()
            || request.run_id().as_uuid() != self.case.id()
            || !matches!(request.original(),PrivateOriginalRef::OriginalPureCaseV1 {case_id,case_checksum}
                if *case_id==self.case.id() && *case_checksum==self.case.checksum())
        {
            return Err(error());
        }
        let row = tx
            .query_one(
                "SELECT case_bytes,checksum FROM reborn_original_pure_cases WHERE case_id=$1",
                &[&self.case.id()],
            )
            .await
            .map_err(|_| error())?;
        if row.get::<_, &str>(0) != self.case.bytes()
            || row.get::<_, Vec<u8>>(1).as_slice() != self.case.checksum()
        {
            return Err(error());
        }
        Ok(())
    }
}
