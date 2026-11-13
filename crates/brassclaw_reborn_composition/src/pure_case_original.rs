//! Immutable authored single-case data bound to I01's checked positive original.
//! This is neither a case oracle, approval, claim nor live execution capability.
use std::sync::Arc;

use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::RetainedComponentSnapshot,
    review_submission_store::{PgReviewSubmissionStore, selection::SourceProofReference},
};
use brassclaw_turns::private_work::PrivateExecutionContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::{
    RebornBuildError,
    bootstrap_reply_validation::invalid,
    private_work_definition::checksum,
    validator_inventory_review::{CheckedPureSourceOriginal, QualifiedPureSourceReview},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Original {
    format: String,
    case_id: Uuid,
    submission_id: Uuid,
    subject_checksum: String,
    source_delivery: SourceProofReference,
    source_confirmation: SourceProofReference,
    source_selection_checksum: String,
    worker_checksum: [u8; 32],
    candidate: ComponentRevisionRef,
    export: String,
    inputs: Value,
    context: PrivateExecutionContext,
}

pub(crate) struct CheckedOriginalPureCase {
    original: Original,
    bytes: String,
    checksum: [u8; 32],
    snapshot: Arc<RetainedComponentSnapshot>,
}
impl CheckedOriginalPureCase {
    /// The exact I01 view, not a caller-provided positive flag, is mandatory.
    pub(crate) async fn capture(
        tx: &Transaction<'_>,
        source: &CheckedPureSourceOriginal,
        context: &PrivateExecutionContext,
        case_id: Uuid,
        export: &str,
        inputs: Value,
    ) -> Result<Self, RebornBuildError> {
        if source.context() != context
            || case_id.is_nil()
            || case_id == source.subject().submission_id()
            || export.is_empty()
            || source.subject().is_function_review()
            || source.subject().candidate().class_code != 22
        {
            return Err(invalid("original Pure case identity is invalid"));
        }
        let original = Original {
            format: "original-pure-case/1".into(),
            case_id,
            submission_id: source.subject().submission_id(),
            subject_checksum: source.subject().subject_checksum().to_owned(),
            source_delivery: source.delivery_reference(),
            source_confirmation: source.confirmation_reference().clone(),
            source_selection_checksum: hex::encode(checksum(source.selection().bytes())),
            worker_checksum: source.worker_checksum(),
            candidate: source.subject().candidate(),
            export: export.to_owned(),
            inputs,
            context: context.clone(),
        };
        let snapshot = Self::snapshot(tx, source).await?;
        let bytes = serde_json::to_string(&original).map_err(invalid)?;
        Ok(Self {
            original,
            checksum: checksum(&bytes),
            bytes,
            snapshot,
        })
    }

    async fn snapshot(
        tx: &Transaction<'_>,
        source: &CheckedPureSourceOriginal,
    ) -> Result<Arc<RetainedComponentSnapshot>, RebornBuildError> {
        let stored =
            PgReviewSubmissionStore::read_in_transaction(tx, source.subject().submission_id())
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("original Pure subject missing"))?;
        if stored.checksum != source.subject().subject_checksum()
            || stored.candidate != source.subject().candidate()
            || stored.function_partner.is_some()
        {
            return Err(invalid("original Pure subject selection differs"));
        }
        Ok(Arc::new(stored.snapshot))
    }

    pub(crate) fn id(&self) -> Uuid {
        self.original.case_id
    }
    pub(crate) fn submission_id(&self) -> Uuid {
        self.original.submission_id
    }
    pub(crate) fn subject_checksum(&self) -> &str {
        &self.original.subject_checksum
    }
    pub(crate) fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub(crate) fn bytes(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn snapshot_ref(&self) -> Arc<RetainedComponentSnapshot> {
        self.snapshot.clone()
    }
    /// Only the candidate's declared executable closure is invoked. A retained
    /// base revision is immutable comparison context, never another definition
    /// to preload merely because it shares the submission snapshot.
    pub(crate) fn executable_components(
        &self,
    ) -> Result<std::collections::BTreeMap<Uuid, ComponentRevisionRef>, RebornBuildError> {
        let mut selected = std::collections::BTreeMap::new();
        let mut pending = vec![self.candidate().uuid];
        while let Some(id) = pending.pop() {
            if selected.contains_key(&id) {
                continue;
            }
            let revision = self
                .snapshot
                .revisions()
                .get(&id)
                .ok_or_else(|| invalid("original Pure executable dependency missing"))?;
            pending.extend(revision.draft().dependencies());
            selected.insert(id, revision.reference());
        }
        Ok(selected)
    }
    pub(crate) fn candidate(&self) -> ComponentRevisionRef {
        self.original.candidate
    }
    pub(crate) fn export(&self) -> &str {
        &self.original.export
    }
    pub(crate) fn inputs(&self) -> &Value {
        &self.original.inputs
    }
    /// Reconstruct only the original declarative Start binding for historical
    /// comparison. This executes no source, opens no worker and issues no live
    /// inspection/approval seal. Defaults use the case's original task profile.
    pub(crate) fn expected_start(
        &self,
        limits: brassclaw_skills::value_contract::ContractLimits,
    ) -> Result<(String, Value), RebornBuildError> {
        use brassclaw_engine::executor::retained_preload::PreloadDeclaration;
        use brassclaw_skills::{
            component_revision::REVISION_LIMITS, value_contract::InputContract,
        };

        let candidate = self
            .snapshot
            .revisions()
            .get(&self.candidate().uuid)
            .ok_or_else(|| invalid("original Pure case candidate missing"))?;
        let document = candidate.draft().document();
        let declaration = PreloadDeclaration::parse(document, candidate.draft().dependencies())
            .map_err(invalid)?
            .ok_or_else(|| invalid("original Pure case export declaration missing"))?;
        let source = declaration
            .invocation(self.export(), &document["input_contract"])
            .map_err(invalid)?;
        let inputs = InputContract::from_value(&document["input_contract"], REVISION_LIMITS)
            .map_err(invalid)?
            .bind_with_limits(self.inputs(), limits)
            .map_err(invalid)?;
        Ok((source, Value::Object(inputs)))
    }
    pub(crate) fn worker_checksum(&self) -> [u8; 32] {
        self.original.worker_checksum
    }
    pub(crate) fn context(&self) -> &PrivateExecutionContext {
        &self.original.context
    }

    /// Join the caller's original case/Work/Ready transaction. Repeated IDs
    /// compare the complete original; they cannot replace inputs or provenance.
    pub(crate) async fn retain(&self, tx: &Transaction<'_>) -> Result<(), RebornBuildError> {
        let context = serde_json::to_value(self.context()).map_err(invalid)?;
        let delivery = hex::decode(&self.original.source_delivery.checksum).map_err(invalid)?;
        let confirmation =
            hex::decode(&self.original.source_confirmation.checksum).map_err(invalid)?;
        tx.execute(
            "INSERT INTO reborn_original_pure_cases(case_id,submission_id,subject_checksum,
            source_work_id,source_delivery_checksum,source_confirmation_checksum,
            source_selection_checksum,execution_context,case_bytes,checksum)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(case_id) DO NOTHING",
            &[
                &self.id(),
                &self.submission_id(),
                &self.original.subject_checksum,
                &self.original.source_delivery.id,
                &delivery,
                &confirmation,
                &self.original.source_selection_checksum,
                &context,
                &self.bytes,
                &&self.checksum[..],
            ],
        )
        .await
        .map_err(invalid)?;
        let row = tx
            .query_one(
                "SELECT case_bytes,checksum FROM reborn_original_pure_cases WHERE case_id=$1",
                &[&self.id()],
            )
            .await
            .map_err(invalid)?;
        if row.get::<_, &str>(0) != self.bytes
            || row.get::<_, Vec<u8>>(1).as_slice() != self.checksum
        {
            return Err(invalid("same original Pure case ID has different bytes"));
        }
        Ok(())
    }

    /// Metadata-only coherent original read. Missing source acknowledgement is
    /// not repaired here and never causes replacement work or native replay.
    pub(crate) async fn read(
        tx: &Transaction<'_>,
        source: &QualifiedPureSourceReview,
        case_id: Uuid,
        context: &PrivateExecutionContext,
    ) -> Result<Option<Self>, RebornBuildError> {
        #[cfg(test)]
        crate::pure_case_settlement::diagnostic::note("read_original_case_bytes");
        let Some(row) = tx.query_opt("SELECT case_bytes,checksum,submission_id,subject_checksum,
            source_work_id,source_delivery_checksum,source_confirmation_checksum,
            source_selection_checksum,execution_context FROM reborn_original_pure_cases WHERE case_id=$1",
            &[&case_id]).await.map_err(invalid)? else { return Ok(None); };
        let bytes: String = row.get(0);
        let expected: [u8; 32] = row
            .get::<_, Vec<u8>>(1)
            .try_into()
            .map_err(|_| invalid("original Pure case checksum invalid"))?;
        let original: Original = serde_json::from_str(&bytes).map_err(invalid)?;
        if checksum(&bytes) != expected
            || original.format != "original-pure-case/1"
            || original.case_id != case_id
            || &original.context != context
        {
            #[cfg(test)]
            crate::pure_case_settlement::diagnostic::note("original_case_bytes_or_context_differs");
            return Err(invalid("original Pure case context or bytes differ"));
        }
        #[cfg(test)]
        crate::pure_case_settlement::diagnostic::note("read_original_positive_source");
        let checked = source
            .read_positive_original(tx, original.submission_id, context)
            .await?
            .ok_or_else(|| {
                invalid("original Pure case positive Source prerequisite unavailable")
            })?;
        #[cfg(test)]
        crate::pure_case_settlement::diagnostic::note("original_positive_source_acknowledged");
        let actual = Self::capture(
            tx,
            &checked,
            context,
            case_id,
            &original.export,
            original.inputs,
        )
        .await?;
        if actual.bytes != bytes
            || actual.checksum != expected
            || row.get::<_, Uuid>(2) != actual.submission_id()
            || row.get::<_, &str>(3) != actual.subject_checksum()
            || row.get::<_, Uuid>(4) != actual.original.source_delivery.id
            || hex::encode(row.get::<_, Vec<u8>>(5)) != actual.original.source_delivery.checksum
            || hex::encode(row.get::<_, Vec<u8>>(6)) != actual.original.source_confirmation.checksum
            || row.get::<_, &str>(7) != actual.original.source_selection_checksum
            || row.get::<_, Value>(8) != serde_json::to_value(context).map_err(invalid)?
        {
            #[cfg(test)]
            crate::pure_case_settlement::diagnostic::note("original_case_projection_differs");
            return Err(invalid("original Pure case projection differs"));
        }
        #[cfg(test)]
        crate::pure_case_settlement::diagnostic::note("original_case_projection_acknowledged");
        Ok(Some(actual))
    }
}
