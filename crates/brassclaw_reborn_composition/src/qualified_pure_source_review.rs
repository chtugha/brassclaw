//! Qualification-bound source inspection of an original authored subject.
//! Python W sequences the checks. This owner authenticates preparation/custody;
//! it never invokes the candidate or grants whole-class Q1, Q2 or activation.
use super::*;
use crate::bootstrap_validator_approval::CheckedPureCheckerClosure;
use crate::installed_monty_catalogue::{InstalledMontyCatalogue, PreparationCapture};
use crate::pg_review_admission_source::PreparedValidatorReview;
use crate::private_work_definition::{
    DefinitionCapture, DomainCapture, PrivateWorkDefinition, decode_checksum,
};
use brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage;
use brassclaw_skills::review_submission_store::selection::SourceProofReference;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{
        PgReviewSubmissionStore, ReviewSubmissionDraft,
        selection::{self, RetainedValidatorSelection, SourceQualification},
    },
    revision_store::PgComponentRevisionStore,
};
use brassclaw_turns::private_work::PrivateExecutionContext;
use brassclaw_turns::{
    PgTurnStateStore, TurnRunId,
    private_work::{
        PrivateOriginalRef, PrivatePreparationReason, PrivateWorkRequest, PrivateWorkStatus,
    },
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

pub(crate) struct QualifiedPureSourceReview {
    workflow: CheckedPureSourceWorkflow,
    checkers: CheckedPureCheckerClosure,
}
pub(crate) struct SourceReviewPreparation {
    catalogue: Arc<InstalledMontyCatalogue>,
    inspected: Arc<InspectedRetainedProgram>,
    preparation: PreparationCapture,
}
/// A single original qualification read, usable only with this owner and the
/// exact still-borrowed transaction. No UUID/global cache or public constructor.
struct CheckedSourceQualification<'checked, 'connection> {
    tx: &'checked Transaction<'connection>,
    owner: &'checked QualifiedPureSourceReview,
    pins: SourceQualification,
    validator_bytes: String,
}
impl CheckedSourceQualification<'_, '_> {
    fn require(
        &self,
        tx: &Transaction<'_>,
        owner: &QualifiedPureSourceReview,
        selected: &RetainedValidatorSelection,
    ) -> Result<(), RebornBuildError> {
        if std::ptr::from_ref(self.tx).cast::<()>() != std::ptr::from_ref(tx).cast::<()>()
            || !std::ptr::eq(self.owner, owner)
            || selected.source_qualification() != Some(&self.pins)
            || selected.validator_bytes() != self.validator_bytes
        {
            return Err(invalid("original qualified Source selection differs"));
        }
        Ok(())
    }
}
/// Positive source-only prerequisite, issued solely by the reader below. This
/// is neither a live native completion nor behavior/Q1/Q2/activation authority.
/// Its references describe the exact original; they do not select replacements.
pub(crate) struct CheckedPureSourceOriginal {
    subject: Arc<RetainedValidatorSubject>,
    selection: Arc<RetainedValidatorSelection>,
    delivery: crate::pg_monty_admission::DeliveredPrivateCompletion,
    confirmation: SourceProofReference,
}
impl CheckedPureSourceOriginal {
    pub(crate) fn context(&self) -> &PrivateExecutionContext {
        self.delivery.original().request().context()
    }
    pub(crate) fn subject(&self) -> &RetainedValidatorSubject {
        &self.subject
    }
    pub(crate) fn selection(&self) -> &RetainedValidatorSelection {
        &self.selection
    }
    pub(crate) fn worker_checksum(&self) -> [u8; 32] {
        self.delivery.original().definition().worker_checksum()
    }
    pub(crate) fn delivery_reference(&self) -> SourceProofReference {
        SourceProofReference {
            id: self.delivery.original().request().work_id(),
            checksum: hex::encode(self.delivery.checksum()),
        }
    }
    pub(crate) fn confirmation_reference(&self) -> &SourceProofReference {
        &self.confirmation
    }
}
impl QualifiedPureSourceReview {
    pub(crate) fn new(
        workflow: CheckedPureSourceWorkflow,
        checkers: CheckedPureCheckerClosure,
    ) -> Self {
        Self { workflow, checkers }
    }
    async fn check_in_transaction<'checked, 'connection>(
        &'checked self,
        tx: &'checked Transaction<'connection>,
    ) -> Result<CheckedSourceQualification<'checked, 'connection>, RebornBuildError> {
        let pins = self
            .workflow
            .source_qualification(tx, &self.checkers)
            .await?;
        let bytes = self
            .workflow
            .program()?
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .exact_bytes()
            .to_owned();
        Ok(CheckedSourceQualification {
            tx,
            owner: self,
            pins,
            validator_bytes: bytes,
        })
    }
    pub(crate) async fn require_selection(
        &self,
        tx: &Transaction<'_>,
        selected: &RetainedValidatorSelection,
    ) -> Result<(), RebornBuildError> {
        self.check_in_transaction(tx)
            .await?
            .require(tx, self, selected)
    }
    pub(crate) fn parser(&self) -> Result<Arc<QualifiedSourceInventoryUsage>, RebornBuildError> {
        self.workflow.parser()
    }
    /// Read the complete exact-positive prerequisite in the caller's coherent
    /// RR snapshot. None means no positively acknowledged original exists yet
    /// (including a genuine negative). Corrupt or incompatible originals error.
    /// This performs no writes, native execution, requalification or recovery.
    /// Cross-package handle/image loading remains the separate recovery owner;
    /// this owner never substitutes today's package for a historical original.
    pub(crate) async fn read_positive_original(
        &self,
        tx: &Transaction<'_>,
        submission_id: Uuid,
        context: &PrivateExecutionContext,
    ) -> Result<Option<CheckedPureSourceOriginal>, RebornBuildError> {
        let subject = Arc::new(
            PgReviewSubmissionStore::read_validator_subject_in_transaction(
                tx,
                submission_id,
                REVISION_LIMITS,
            )
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("Source prerequisite subject missing"))?,
        );
        let selection = Arc::new(
            selection::read_preparation_in_transaction(tx, submission_id, REVISION_LIMITS)
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("Source prerequisite selection missing"))?,
        );
        if subject.candidate().class_code != 22 || selection.source_qualification().is_none() {
            return Err(invalid("qualified Source prerequisite required"));
        }
        let Some(delivery) = PgMontyAdmission::read_private_delivery(tx, submission_id, context)
            .await
            .map_err(invalid)?
        else {
            return Ok(None);
        };
        crate::pg_review_admission_source::ValidatorInputCapture::verify_historical(
            &subject,
            &selection,
            delivery.original().definition(),
        )?;
        if selection.source_qualification().is_none_or(|q| {
            q.worker_checksum != hex::encode(delivery.original().definition().worker_checksum())
        }) {
            return Err(invalid("Source prerequisite original worker differs"));
        }
        // H01 authenticates the actual delivered verdict and reconstructs and
        // byte-compares its original /1 acknowledgement. No JSON flag by itself
        // creates the checked view, and no missing confirmation is produced.
        let Some(record) = crate::review_work_confirmation::read_with_source(
            tx,
            submission_id,
            context,
            Some(self),
        )
        .await
        .map_err(invalid)?
        else {
            return Ok(None);
        };
        if record["format"] != "component-review-work-confirmation/1"
            || record["scope"] != "structural-observations-only"
            || record["q1_qualified"] != false
            || record["human_q2"] != false
            || record["activation"] != false
        {
            return Err(invalid("Source prerequisite confirmation scope differs"));
        }
        match record["checks_succeeded"].as_bool() {
            Some(false) => return Ok(None),
            Some(true) if record["errors"] == json!([]) => {}
            _ => return Err(invalid("Source prerequisite positive verdict differs")),
        }
        let row = tx.query_opt("SELECT confirmation_bytes,checksum FROM reborn_component_review_work_confirmations WHERE submission_id=$1", &[&submission_id])
            .await.map_err(invalid)?.ok_or_else(|| invalid("Source prerequisite original confirmation missing"))?;
        let bytes: &str = row.get(0);
        let checksum: [u8; 32] = row
            .get::<_, Vec<u8>>(1)
            .try_into()
            .map_err(|_| invalid("Source prerequisite confirmation checksum invalid"))?;
        if bytes != serde_json::to_string(&record).map_err(invalid)?
            || crate::private_work_definition::checksum(bytes) != checksum
            || record["delivery_checksum"]
                != serde_json::to_value(delivery.checksum()).map_err(invalid)?
        {
            return Err(invalid("Source prerequisite original confirmation differs"));
        }
        Ok(Some(CheckedPureSourceOriginal {
            subject,
            selection,
            delivery,
            confirmation: SourceProofReference {
                id: submission_id,
                checksum: hex::encode(checksum),
            },
        }))
    }
}
impl InstalledMontyCatalogue {
    /// Reinspect only the already qualified W, outside the submission transaction.
    /// The submitted candidate is never an executable part of this programme.
    pub(crate) async fn prepare_source_submission(
        self: &Arc<Self>,
        draft: &ReviewSubmissionDraft,
    ) -> Result<Option<SourceReviewPreparation>, RebornBuildError> {
        if draft.candidate().class_code != 22 || draft.is_function_review() {
            return Ok(None);
        }
        // Historical same-ID requests must not capture today's settings or
        // inspect new definitions. The transaction later compares exact bytes.
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            self.pool.get().await.map_err(invalid)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let existing = PgReviewSubmissionStore::read_in_transaction(&tx, draft.submission_id())
            .await
            .map_err(invalid)?
            .is_some();
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        drop(client);
        if existing {
            return Ok(None);
        }
        let preparation = match draft.preparation() {
            Some(original) => {
                PreparationCapture::from_original(original.clone()).map_err(invalid)?
            }
            None => self.capture_task_preparation().await?,
        };
        let worker = verified_worker(
            &self.worker_image,
            preparation.limits().max_native_artifact_bytes,
        )
        .await?;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(self.pure_source_review.workflow.program()?),
                &worker.path(),
                preparation.limits(),
            )
            .await
            .map_err(invalid)?,
        );
        #[cfg(test)]
        if let Some((captured, release)) = self.capture_gate.lock().await.take() {
            captured.notify_one();
            release.notified().await;
        }
        Ok(Some(SourceReviewPreparation {
            catalogue: self.clone(),
            inspected,
            preparation,
        }))
    }
    /// Reuse the exact retained qualification before interpreting any /2 input
    /// or historical evidence. A /1 preparation keeps its original scope.
    pub(crate) async fn require_source_selection(
        &self,
        tx: &Transaction<'_>,
        selected: &RetainedValidatorSelection,
    ) -> Result<(), RebornBuildError> {
        if selected.source_qualification().is_some() {
            self.pure_source_review
                .require_selection(tx, selected)
                .await?;
        }
        Ok(())
    }
    /// Strict metadata readback, including Ready/claimed originals. It never
    /// fills a missing preparation or reopens execution after an uncertain ACK.
    pub(crate) async fn verify_source_submission(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<(), RebornBuildError> {
        self.verify_source_submission_checked(tx, id, None).await
    }
    async fn verify_source_submission_checked(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
        checked: Option<&CheckedSourceQualification<'_, '_>>,
    ) -> Result<(), RebornBuildError> {
        let Some(selected) = selection::read_preparation_in_transaction(tx, id, REVISION_LIMITS)
            .await
            .map_err(invalid)?
        else {
            return if checked.is_some() {
                Err(invalid("checked Source preparation disappeared"))
            } else {
                Ok(())
            };
        };
        if selected.source_qualification().is_none() {
            return if checked.is_some() {
                Err(invalid("checked Source qualification disappeared"))
            } else {
                Ok(())
            };
        }
        if let Some(checked) = checked {
            checked.require(tx, &self.pure_source_review, &selected)?;
        } else {
            self.require_source_selection(tx, &selected).await?;
        }
        let subject =
            PgReviewSubmissionStore::read_validator_subject_in_transaction(tx, id, REVISION_LIMITS)
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("Source original missing"))?;
        let row = tx.query_opt("SELECT w.private_request_bytes,w.private_request_checksum,p.definition_bytes,p.checksum,s.actor FROM brassclaw_turns w JOIN brassclaw_monty_private_preparations p ON p.work_id=w.work_id AND p.checksum=w.private_definition_checksum JOIN reborn_component_review_submissions s ON s.submission_id=w.validator_submission_id WHERE w.work_id=$1 AND w.record_kind='private_work'", &[&id]).await.map_err(invalid)?.ok_or_else(|| invalid("Source original Work/capture missing"))?;
        let request: PrivateWorkRequest = serde_json::from_str(row.get(0)).map_err(invalid)?;
        let checksum: [u8; 32] = row
            .get::<_, Vec<u8>>(1)
            .try_into()
            .map_err(|_| invalid("Source request digest invalid"))?;
        if crate::private_work_definition::checksum(row.get(0)) != checksum
            || !matches!(request.original(), PrivateOriginalRef::ValidatorStructural {submission_id,subject_checksum} if *submission_id==id && hex::encode(subject_checksum)==subject.subject_checksum())
            || request.context().actor.as_ref().map(|a| a.user_id.as_str())
                != Some(row.get::<_, &str>(4))
        {
            return Err(invalid("Source original request differs"));
        }
        let current = self.source_work_context()?;
        if current.tenant_id != request.context().tenant_id
            || current.agent_id != request.context().agent_id
            || current.project_id != request.context().project_id
        {
            return Err(invalid("Source original instance context differs"));
        }
        let state = PgTurnStateStore::read_private_work_state_in_transaction(
            tx,
            current.tenant_id.as_str(),
            id,
        )
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("Source original Work state missing"))?;
        if state.request != request
            || state.request_checksum != checksum
            || state.status == PrivateWorkStatus::AwaitingPreparation
        {
            return Err(invalid("Source original Ready seal differs"));
        }
        use brassclaw_turns::pg_turn_state_store::PgPrivateOriginalVerifier;
        self.verify_original(tx, &request).await.map_err(invalid)?;
        let definition = PrivateWorkDefinition::restore(
            tx,
            &request,
            checksum,
            row.get(2),
            row.get::<_, Vec<u8>>(3)
                .try_into()
                .map_err(|_| invalid("Source definition digest invalid"))?,
        )
        .await?;
        crate::pg_review_admission_source::ValidatorInputCapture::verify_historical(
            &subject,
            &selected,
            &definition,
        )?;
        if selected
            .source_qualification()
            .is_none_or(|q| q.worker_checksum != hex::encode(definition.worker_checksum()))
        {
            return Err(invalid("Source original worker differs from qualification"));
        }
        Ok(())
    }
    pub(crate) async fn read_source_submission(&self, id: Uuid) -> Result<(), RebornBuildError> {
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            self.pool.get().await.map_err(invalid)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        self.verify_source_submission(&tx, id).await?;
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(())
    }
}
impl SourceReviewPreparation {
    pub(crate) async fn verify_original(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<(), RebornBuildError> {
        self.catalogue.verify_source_submission(tx, id).await
    }
    pub(crate) fn notify(&self) {
        if let Some(wake) = self.catalogue.private_work_wake.get() {
            wake.wake();
        }
    }
    /// Joins the existing submission transaction after its immutable subject
    /// and queue binding have been sealed and re-read. No native I/O occurs.
    pub(crate) async fn retain(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<(), RebornBuildError> {
        let subject = Arc::new(
            PgReviewSubmissionStore::read_validator_subject_in_transaction(tx, id, REVISION_LIMITS)
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("Source original subject missing"))?,
        );
        if subject.candidate().class_code != 22 {
            return Err(invalid("Source subject class differs"));
        }
        if tx.query_opt("SELECT submission_id FROM reborn_component_validator_selections WHERE submission_id=$1", &[&id]).await.map_err(invalid)?.is_some() {
            // A concurrent identical submission may already have sealed Ready.
            // Reuse its original preparation; never replace it with this capture.
            return self.verify_original(tx, id).await;
        }
        let checked = self
            .catalogue
            .pure_source_review
            .check_in_transaction(tx)
            .await?;
        let coverage = RetainedValidatorCoverage::from_inputs(self.inspected.program().inputs())
            .map_err(invalid)?;
        let mut value: Value = serde_json::from_str(
            &RetainedValidatorCoverage::selection_bytes(&subject, &self.inspected, REVISION_LIMITS)
                .map_err(invalid)?,
        )
        .map_err(invalid)?;
        value["format"] = json!("component-validator-selection/2");
        value["scope"] = json!("qualified-pure-source-prerequisite");
        value["qualification"] = serde_json::to_value(&checked.pins).map_err(invalid)?;
        selection::retain_in_transaction(tx, &value.to_string(), REVISION_LIMITS)
            .await
            .map_err(invalid)?;
        let selected = Arc::new(
            selection::read_preparation_in_transaction(tx, id, REVISION_LIMITS)
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("Source selection missing"))?,
        );
        // retain_in_transaction compared the complete encoded outer bytes;
        // the qualification above already authenticated their original proof
        // under this same RR view. The final Ready reread below checks custody.
        let prepared = PreparedValidatorReview::capture_qualified(
            subject.clone(),
            selected,
            self.inspected.clone(),
            coverage,
            self.preparation.clone(),
            self.catalogue.worker_image.checksum(),
        )?;
        let mut context = self.catalogue.source_work_context()?;
        let actor: String = tx
            .query_one(
                "SELECT actor FROM reborn_component_review_submissions WHERE submission_id=$1",
                &[&id],
            )
            .await
            .map_err(invalid)?
            .get(0);
        context.actor = Some(brassclaw_turns::TurnActor::new(
            brassclaw_host_api::UserId::new(actor).map_err(invalid)?,
        ));
        let capture = prepared.input_capture();
        let request = PrivateWorkRequest::V1 {
            work_id: id,
            run_id: TurnRunId::from_uuid(id),
            original: PrivateOriginalRef::ValidatorStructural {
                submission_id: id,
                subject_checksum: decode_checksum(subject.subject_checksum())?,
            },
            context: context.clone(),
            original_preparation_checksum: Some(decode_checksum(capture.preparation().checksum())?),
            original_workflow_checksum: Some(decode_checksum(capture.validator_checksum())?),
        };
        let store = PgTurnStateStore::new(self.catalogue.pool.clone(), context.tenant_id.as_str());
        let request_checksum = store
            .submit_private_work_in_transaction(
                tx,
                &request,
                PrivatePreparationReason::PreparationPending,
                self.catalogue.as_ref(),
            )
            .await
            .map_err(invalid)?;
        let state = store
            .get_private_work_state_in_transaction(tx, id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("Source Work missing"))?;
        if state.status != PrivateWorkStatus::AwaitingPreparation {
            return Err(invalid("Source original Work already sealed"));
        }
        let root = PgComponentRevisionStore::read_revision_in_transaction(
            tx,
            self.catalogue.root_reference,
        )
        .await
        .map_err(invalid)?;
        let definition = PrivateWorkDefinition::capture(DefinitionCapture {
            request: &request,
            request_checksum,
            preparation: self.preparation.clone(),
            worker_checksum: self.catalogue.worker_image.checksum(),
            root: &root,
            workflow_checksum: decode_checksum(capture.validator_checksum())?,
            input_checksum: decode_checksum(capture.input_checksum())?,
            domain: DomainCapture::ValidatorStructural {
                capture_bytes: capture.bytes().to_owned(),
                capture_checksum: decode_checksum(capture.checksum())?,
            },
        })?;
        self.catalogue
            .retain_private_definition(tx, &definition, &store, &state)
            .await?;
        self.catalogue
            .verify_source_submission_checked(tx, id, Some(&checked))
            .await
    }
}
