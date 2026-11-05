//! Immutable authoring subjects. Retention supplies neither review evidence nor
//! activation: a validator Recipe must inspect this exact graph separately.

use std::{collections::BTreeSet, sync::Arc};

use brassclaw_pg::{PgError, PgPool};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

#[path = "review_selection_store.rs"]
pub mod selection;

#[path = "review_queue_binding.rs"]
pub mod queue;

#[path = "review_function_subject.rs"]
mod function;
use function::FunctionPartner;
pub use function::FunctionReviewPartnerDraft;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{
        ComponentRevisionDraft, RetainedComponentRevision, RetainedComponentSnapshot,
    },
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
    value_contract::{ContractLimits, strict_json, validate_data_bounds},
};

const MAX_SUBJECT_BYTES: usize = 16_777_216;
const MAX_DEPENDENCIES: usize = 4095;

#[derive(thiserror::Error)]
pub enum SubmissionStoreError {
    #[error("review submission database operation requires exact retry")]
    Database(#[source] PgError),
    #[error("review submission identity or base revision conflict")]
    Conflict,
    #[error("invalid immutable review subject")]
    Invalid,
    #[error("new class-50 review submissions are not supported; use a normal Recipe")]
    RetiredScaffold,
    #[error("review subject exceeds technical transport capacity")]
    Capacity,
    #[error(transparent)]
    Revisions(#[from] RevisionStoreError),
}
impl std::fmt::Debug for SubmissionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}
fn database(error: impl Into<PgError>) -> SubmissionStoreError {
    SubmissionStoreError::Database(error.into())
}
fn hex(checksum: [u8; 32]) -> String {
    checksum.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    uuid: Uuid,
    class_code: i32,
    version: u64,
    checksum: String,
}
impl From<ComponentRevisionRef> for Reference {
    fn from(value: ComponentRevisionRef) -> Self {
        Self {
            uuid: value.uuid,
            class_code: value.class_code,
            version: value.version,
            checksum: hex(value.checksum),
        }
    }
}
impl Reference {
    fn parsed(&self) -> Result<ComponentRevisionRef, SubmissionStoreError> {
        if self.uuid.is_nil()
            || self.version == 0
            || self.version > i64::MAX as u64
            || !matches!(self.class_code, 0..=10 | 12..=23 | 50)
            || self.checksum.len() != 64
            || !self
                .checksum
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(SubmissionStoreError::Invalid);
        }
        let mut checksum = [0; 32];
        for (slot, pair) in checksum
            .iter_mut()
            .zip(self.checksum.as_bytes().as_chunks::<2>().0.iter())
        {
            let digit = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
            *slot = digit(pair[0]) * 16 + digit(pair[1]);
        }
        Ok(ComponentRevisionRef {
            uuid: self.uuid,
            class_code: self.class_code,
            version: self.version,
            checksum,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    format: String,
    submission_id: Uuid,
    actor: String,
    candidate: Reference,
    candidate_bytes: String,
    base: Option<Reference>,
    dependencies: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected_previous_version: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    function_partner: Option<FunctionPartner>,
}

/// Validated proposal, with all transitive dependency revisions selected by the
/// author. The graph may contain drafts; this says nothing about their approval.
/// No Debug implementation exposes candidate content or actor data.
pub struct ReviewSubmissionDraft {
    id: Uuid,
    actor: String,
    candidate: ComponentRevisionDraft,
    reference: ComponentRevisionRef,
    base: Option<ComponentRevisionRef>,
    dependencies: Vec<ComponentRevisionRef>,
    bytes: String,
    checksum: String,
    expected_previous_version: u64,
    function_partner: Option<FunctionPartner>,
}
impl ReviewSubmissionDraft {
    pub fn submission_id(&self) -> Uuid {
        self.id
    }
    pub fn candidate(&self) -> ComponentRevisionRef {
        self.reference
    }
    pub fn candidate_document(&self) -> &serde_json::Value {
        self.candidate.document()
    }
    pub fn changed_roots(&self) -> Result<Vec<ComponentRevisionRef>, SubmissionStoreError> {
        let mut roots = vec![self.reference];
        if let Some(partner) = &self.function_partner
            && partner.is_changed()
        {
            roots.push(partner.reference()?);
        }
        roots.sort_by_key(|r| r.uuid);
        Ok(roots)
    }

    /// Original documents for changed roots only. Unchanged partners remain
    /// exact context, never additional queued/approval candidates.
    pub fn changed_root_documents(
        &self,
    ) -> Result<Vec<(ComponentRevisionRef, serde_json::Value)>, SubmissionStoreError> {
        let mut documents = vec![(self.reference, self.candidate.document().clone())];
        if let Some(partner) = &self.function_partner
            && let FunctionReviewPartnerDraft::Changed { candidate, .. } = partner.to_draft()?
        {
            documents.push((partner.reference()?, candidate.document().clone()));
        }
        documents.sort_by_key(|(reference, _)| reference.uuid);
        Ok(documents)
    }

    pub fn new(
        id: Uuid,
        actor: &str,
        candidate: ComponentRevisionDraft,
        base: Option<ComponentRevisionRef>,
        dependencies: Vec<ComponentRevisionRef>,
    ) -> Result<Self, SubmissionStoreError> {
        Self::build(id, actor, candidate, base, dependencies, None, None)
    }

    /// The review base is lineage, not the allocation counter. A package draft
    /// or another unactivated proposal may have allocated intervening versions.
    /// Callers must observe the head and retain it exactly for uncertain retries.
    pub fn new_after_head(
        id: Uuid,
        actor: &str,
        candidate: ComponentRevisionDraft,
        base: Option<ComponentRevisionRef>,
        dependencies: Vec<ComponentRevisionRef>,
        expected_previous_version: u64,
    ) -> Result<Self, SubmissionStoreError> {
        Self::build(
            id,
            actor,
            candidate,
            base,
            dependencies,
            Some(expected_previous_version),
            None,
        )
    }

    /// One function review, with either two changed roots or one changed root
    /// and an exact unchanged partner. The latter is context, not reapproval.
    /// No candidate partner must already be approved merely to retain a draft.
    pub fn new_function(
        id: Uuid,
        actor: &str,
        candidate: ComponentRevisionDraft,
        base: Option<ComponentRevisionRef>,
        dependencies: Vec<ComponentRevisionRef>,
        expected_previous_version: u64,
        partner: FunctionReviewPartnerDraft,
    ) -> Result<Self, SubmissionStoreError> {
        let partner = FunctionPartner::from_draft(partner)?;
        Self::build(
            id,
            actor,
            candidate,
            base,
            dependencies,
            Some(expected_previous_version),
            Some(partner),
        )
    }

    fn build(
        id: Uuid,
        actor: &str,
        candidate: ComponentRevisionDraft,
        base: Option<ComponentRevisionRef>,
        mut dependencies: Vec<ComponentRevisionRef>,
        observed_head: Option<u64>,
        function_partner: Option<FunctionPartner>,
    ) -> Result<Self, SubmissionStoreError> {
        if id.is_nil() || actor.trim().is_empty() || actor.len() > 2048 {
            return Err(SubmissionStoreError::Invalid);
        }
        if dependencies.len() > MAX_DEPENDENCIES - usize::from(function_partner.is_some()) {
            return Err(SubmissionStoreError::Capacity);
        }
        let expected_previous_version =
            observed_head.unwrap_or_else(|| base.map_or(0, |base| base.version));
        let reference = candidate_reference(&candidate, base, expected_previous_version)?;
        let partner_ref = function_partner
            .as_ref()
            .map(FunctionPartner::reference)
            .transpose()?;
        if let Some(partner) = partner_ref
            && (observed_head.is_none()
                || partner.uuid == reference.uuid
                || !matches!(
                    (reference.class_code, partner.class_code),
                    (1..=3, 22) | (22, 1..=3)
                ))
        {
            return Err(SubmissionStoreError::Invalid);
        }
        let mut unique = BTreeSet::new();
        for dependency in &dependencies {
            Reference::from(*dependency).parsed()?;
            if dependency.uuid == reference.uuid
                || partner_ref.is_some_and(|partner| partner.uuid == dependency.uuid)
                || !unique.insert(dependency.uuid)
            {
                return Err(SubmissionStoreError::Invalid);
            }
        }
        dependencies.sort();
        let subject = Subject {
            format: if function_partner.is_some() {
                "component-review-submission/3"
            } else if observed_head.is_some() {
                "component-review-submission/2"
            } else {
                "component-review-submission/1"
            }
            .into(),
            submission_id: id,
            actor: actor.into(),
            candidate: reference.into(),
            candidate_bytes: candidate.exact_bytes().into(),
            base: base.map(Into::into),
            dependencies: dependencies.iter().copied().map(Into::into).collect(),
            expected_previous_version: observed_head,
            function_partner: function_partner.clone(),
        };
        let bytes = serde_json::to_string(&subject).map_err(|_| SubmissionStoreError::Invalid)?;
        if bytes.len() > MAX_SUBJECT_BYTES {
            return Err(SubmissionStoreError::Capacity);
        }
        let checksum = hex(Sha256::digest(bytes.as_bytes()).into());
        Ok(Self {
            id,
            actor: actor.into(),
            candidate,
            reference,
            base,
            dependencies,
            bytes,
            checksum,
            expected_previous_version,
            function_partner,
        })
    }
}

fn candidate_reference(
    candidate: &ComponentRevisionDraft,
    base: Option<ComponentRevisionRef>,
    expected_previous_version: u64,
) -> Result<ComponentRevisionRef, SubmissionStoreError> {
    if let Some(base) = base {
        Reference::from(base).parsed()?;
        if base.uuid != candidate.uuid() || base.class_code != candidate.class_code() {
            return Err(SubmissionStoreError::Invalid);
        }
    }
    if base.is_none() && expected_previous_version != 0
        || base.is_some_and(|base| base.version > expected_previous_version)
    {
        return Err(SubmissionStoreError::Invalid);
    }
    let version = expected_previous_version
        .checked_add(1)
        .filter(|version| *version <= i64::MAX as u64)
        .ok_or(SubmissionStoreError::Invalid)?;
    Ok(ComponentRevisionRef {
        uuid: candidate.uuid(),
        class_code: candidate.class_code(),
        version,
        checksum: candidate.checksum(),
    })
}

/// Retained role and exact identity. Context does not assert prior approval.
#[derive(Clone, Copy)]
pub struct RetainedFunctionPartner {
    pub reference: ComponentRevisionRef,
    pub changed: bool,
    pub base: Option<ComponentRevisionRef>,
}

pub struct RetainedReviewSubmission {
    pub id: Uuid,
    pub actor: String,
    pub candidate: ComponentRevisionRef,
    pub function_partner: Option<RetainedFunctionPartner>,
    pub base: Option<ComponentRevisionRef>,
    pub dependencies: Vec<ComponentRevisionRef>,
    pub subject_bytes: String,
    pub checksum: String,
    pub snapshot: RetainedComponentSnapshot,
}

/// Durable review intent read from the submission's exact immutable link. It
/// contains no selected validator, claim, attempt, verdict or approval grant.
pub struct RetainedReviewRequest {
    submission_id: Uuid,
    subject_checksum: String,
}
impl RetainedReviewRequest {
    pub fn submission_id(&self) -> Uuid {
        self.submission_id
    }
    pub fn subject_checksum(&self) -> &str {
        &self.subject_checksum
    }
}

/// Complete typed input read from immutable storage, never a validator verdict.
/// Private construction prevents caller-supplied success JSON from masquerading
/// as a trusted subject. Source text remains data inside exact revision bytes.
pub struct RetainedValidatorSubject {
    value: serde_json::Value,
    candidate: ComponentRevisionRef,
    submission_id: Uuid,
    subject_checksum: String,
    function_review: bool,
}
impl RetainedValidatorSubject {
    pub fn value(&self) -> &serde_json::Value {
        &self.value
    }
    pub fn candidate(&self) -> ComponentRevisionRef {
        self.candidate
    }
    /// Single-class coverage cannot establish complete joint function review.
    pub fn is_function_review(&self) -> bool {
        self.function_review
    }
    /// Original immutable submission identity, not a review or task approval.
    pub fn submission_id(&self) -> Uuid {
        self.submission_id
    }
    /// Covers the original candidate, lineage base and exact dependency choice.
    /// It is distinct from the validator program's own selection checksum.
    pub fn subject_checksum(&self) -> &str {
        &self.subject_checksum
    }
}

fn revision_input(
    revision: &RetainedComponentRevision,
    limits: ContractLimits,
) -> Result<serde_json::Value, SubmissionStoreError> {
    // The entire envelope is typed data, including declared dependencies and
    // exact association bytes. Projecting just `document` loses those fields.
    let envelope = strict_json(revision.draft().exact_bytes(), limits)
        .map_err(|_| SubmissionStoreError::Capacity)?;
    Ok(serde_json::json!({
        "reference": Reference::from(revision.reference()),
        "revision_bytes": revision.draft().exact_bytes(),
        "revision": envelope,
    }))
}

#[derive(Clone)]
pub struct PgReviewSubmissionStore {
    pool: Arc<PgPool>,
}

enum CandidateRetention {
    Stage,
    Existing,
}

impl PgReviewSubmissionStore {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// A stable ID plus identical subject recovers an unknown commit before
    /// attempting a new version allocation. Changed bytes/actor/base conflict.
    pub async fn submit(
        &self,
        draft: &ReviewSubmissionDraft,
    ) -> Result<RetainedReviewSubmission, SubmissionStoreError> {
        let mut client = self.pool.get().await.map_err(database)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(database)?;
        let retained = Self::submit_in_transaction(&tx, draft).await?;
        tx.commit().await.map_err(database)?;
        Ok(retained)
    }

    /// Attach a review to an already retained exact draft, without allocating
    /// another revision or moving its head. The subject still validates its
    /// base, complete selected graph and exact stored bytes. Identical retry
    /// recovers a committed submission; a conflicting/concurrent write errors
    /// and must be retried with the same subject, never a changed candidate.
    pub async fn submit_retained(
        &self,
        draft: &ReviewSubmissionDraft,
    ) -> Result<RetainedReviewSubmission, SubmissionStoreError> {
        let mut client = self.pool.get().await.map_err(database)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(database)?;
        let retained = Self::retain_subject(&tx, draft, CandidateRetention::Existing).await?;
        tx.commit().await.map_err(database)?;
        Ok(retained)
    }

    /// Join an author-owned repeatable-read transaction. The owner must roll
    /// back on error and acknowledge success only after the actual commit.
    pub async fn submit_in_transaction(
        tx: &Transaction<'_>,
        draft: &ReviewSubmissionDraft,
    ) -> Result<RetainedReviewSubmission, SubmissionStoreError> {
        Self::retain_subject(tx, draft, CandidateRetention::Stage).await
    }

    async fn retain_subject(
        tx: &Transaction<'_>,
        draft: &ReviewSubmissionDraft,
        retention: CandidateRetention,
    ) -> Result<RetainedReviewSubmission, SubmissionStoreError> {
        require_snapshot(tx).await?;
        if let Some(existing) = read(tx, draft.id).await? {
            if existing.subject_bytes != draft.bytes || existing.checksum != draft.checksum {
                return Err(SubmissionStoreError::Conflict);
            }
            return Ok(existing);
        }
        // Historical bytes and identical original retries stay readable above.
        // Only a fresh primary review is rejected, before either retention mode
        // can allocate a revision/head or create a V149 review request. Keep
        // class-10 root review and exact historical dependency/base refs intact.
        if draft.candidate.class_code() == 50 {
            return Err(SubmissionStoreError::RetiredScaffold);
        }
        let mut changes = vec![(
            &draft.candidate,
            draft.base,
            draft.expected_previous_version,
            draft.reference,
        )];
        let partner_draft = draft
            .function_partner
            .as_ref()
            .map(FunctionPartner::to_draft)
            .transpose()?;
        if let Some(FunctionReviewPartnerDraft::Changed {
            candidate,
            base,
            expected_previous_version,
        }) = &partner_draft
        {
            changes.push((
                candidate,
                *base,
                *expected_previous_version,
                draft
                    .function_partner
                    .as_ref()
                    .ok_or(SubmissionStoreError::Invalid)?
                    .reference()?,
            ));
        }
        // Consistent lock order for opposite primary/partner orderings. A failure
        // rolls back both allocations; never acknowledge the first staged root.
        changes.sort_by_key(|(candidate, _, _, _)| candidate.uuid());
        for (candidate, base, head, expected) in changes {
            if let Some(base) = base {
                PgComponentRevisionStore::read_revision_in_transaction(tx, base)
                    .await
                    .map_err(|error| match error {
                        // The operator's exact lineage selection conflicts with
                        // retained identity/bytes. Preserve the original 409
                        // contract for either changed root, before allocation.
                        RevisionStoreError::Integrity => SubmissionStoreError::Conflict,
                        other => SubmissionStoreError::Revisions(other),
                    })?;
            }
            let actual = match retention {
                CandidateRetention::Stage => {
                    PgComponentRevisionStore::stage_in_transaction(tx, candidate, head).await?
                }
                CandidateRetention::Existing => {
                    PgComponentRevisionStore::read_revision_in_transaction(tx, expected).await?;
                    expected
                }
            };
            if actual != expected {
                return Err(SubmissionStoreError::Invalid);
            }
        }
        let staged = draft.reference;
        let (roots, references) = subject_references(draft)?;
        let snapshot =
            PgComponentRevisionStore::read_exact_in_transaction(tx, &roots, &references).await?;
        let function_partner = retained_partner(draft, &snapshot)?;
        let version = staged.version as i64;
        let base_version = draft.base.map(|base| base.version as i64);
        tx.execute("INSERT INTO reborn_component_review_submissions (submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum) VALUES ($1,$2,$3,$4,$5,$6,$7)",
            &[&draft.id, &draft.actor, &staged.uuid, &version, &base_version, &draft.bytes, &draft.checksum]).await.map_err(database)?;
        verify_request(tx, draft.id, &draft.checksum).await?;
        Ok(RetainedReviewSubmission {
            id: draft.id,
            actor: draft.actor.clone(),
            candidate: staged,
            function_partner,
            base: draft.base,
            dependencies: draft.dependencies.clone(),
            subject_bytes: draft.bytes.clone(),
            checksum: draft.checksum.clone(),
            snapshot,
        })
    }

    /// Exact immutable subject read in the caller's coherent catalogue snapshot.
    /// This verifies retention/integrity, never grants component approval.
    pub async fn read_in_transaction(
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<Option<RetainedReviewSubmission>, SubmissionStoreError> {
        if id.is_nil() {
            return Err(SubmissionStoreError::Invalid);
        }
        require_snapshot(tx).await?;
        read(tx, id).await
    }

    /// Resolve one work reference in the same snapshot as the complete subject
    /// and validator selection. This never claims work or invokes a scheduler.
    pub async fn read_review_request_in_transaction(
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<Option<RetainedReviewRequest>, SubmissionStoreError> {
        if id.is_nil() {
            return Err(SubmissionStoreError::Invalid);
        }
        require_snapshot(tx).await?;
        let Some(row) = tx.query_opt(
            "SELECT s.checksum,r.subject_checksum,s.checksum=encode(sha256(convert_to(s.subject_bytes,'UTF8')),'hex')
             FROM reborn_component_review_submissions s
             LEFT JOIN reborn_component_review_requests r USING(submission_id)
             WHERE s.submission_id=$1", &[&id],
        ).await.map_err(database)? else {
            return Ok(None);
        };
        let checksum: String = row.get(0);
        if row.get::<_, Option<String>>(1).as_deref() != Some(checksum.as_str())
            || !row.get::<_, bool>(2)
        {
            return Err(SubmissionStoreError::Invalid);
        }
        Ok(Some(RetainedReviewRequest {
            submission_id: id,
            subject_checksum: checksum,
        }))
    }

    /// Prepare the full proposed graph for a privately selected validator Recipe
    /// in the same snapshot as its selection. Limits come from the acknowledged
    /// preparation policy; exceeding them rejects the whole input, never trims it.
    /// The base is comparison-only: submissions retain its exact root reference,
    /// not its historical dependency-version graph or an approval receipt.
    pub async fn read_validator_subject_in_transaction(
        tx: &Transaction<'_>,
        id: Uuid,
        limits: ContractLimits,
    ) -> Result<Option<RetainedValidatorSubject>, SubmissionStoreError> {
        validate_data_bounds(&serde_json::Value::Null, limits)
            .map_err(|_| SubmissionStoreError::Capacity)?;
        let Some(subject) = Self::read_in_transaction(tx, id).await? else {
            return Ok(None);
        };
        let candidate = subject
            .snapshot
            .revisions()
            .get(&subject.candidate.uuid)
            .ok_or(SubmissionStoreError::Invalid)?;
        let base = match subject.base {
            Some(reference) => {
                Some(PgComponentRevisionStore::read_revision_in_transaction(tx, reference).await?)
            }
            None => None,
        };
        let partner_base = match subject.function_partner.and_then(|partner| partner.base) {
            Some(reference) => {
                Some(PgComponentRevisionStore::read_revision_in_transaction(tx, reference).await?)
            }
            None => None,
        };
        // Exact text is necessarily present in the eventual envelope. Reject
        // this lower bound before cloning/parsing all revisions for transport.
        subject
            .snapshot
            .revisions()
            .values()
            .chain(base.iter())
            .chain(partner_base.iter())
            .try_fold(subject.subject_bytes.len(), |bytes, revision| {
                bytes.checked_add(revision.draft().exact_bytes().len())
            })
            .filter(|bytes| *bytes <= limits.max_bytes)
            .ok_or(SubmissionStoreError::Capacity)?;
        // Check borrowed documents before duplicating them into transport data.
        for revision in subject
            .snapshot
            .revisions()
            .values()
            .chain(base.iter())
            .chain(partner_base.iter())
        {
            validate_data_bounds(revision.draft().document(), limits)
                .map_err(|_| SubmissionStoreError::Capacity)?;
        }
        let dependencies = subject
            .snapshot
            .revisions()
            .values()
            .filter(|revision| {
                revision.reference().uuid != subject.candidate.uuid
                    && subject
                        .function_partner
                        .is_none_or(|partner| partner.reference.uuid != revision.reference().uuid)
            })
            .map(|revision| revision_input(revision, limits))
            .collect::<Result<Vec<_>, _>>()?;
        let submission_id = subject.id;
        let subject_checksum = subject.checksum.clone();
        let mut value = serde_json::json!({
            "format": if subject.function_partner.is_some() { "component-validator-subject/2" } else { "component-validator-subject/1" },
            "submission_id": subject.id,
            "actor": subject.actor,
            "subject_bytes": subject.subject_bytes,
            "subject_checksum": subject.checksum,
            "candidate": revision_input(candidate, limits)?,
            "dependencies": dependencies,
            "base_comparison": base.as_ref().map(|revision| revision_input(revision, limits)).transpose()?,
        });
        if let Some(partner) = subject.function_partner {
            let revision = subject
                .snapshot
                .revisions()
                .get(&partner.reference.uuid)
                .ok_or(SubmissionStoreError::Invalid)?;
            value["function_partner"] = serde_json::json!({
                "role": if partner.changed { "changed" } else { "context" },
                "component": revision_input(revision, limits)?,
                "base_comparison": partner_base.as_ref().map(|revision| revision_input(revision, limits)).transpose()?,
            });
        }
        validate_data_bounds(&value, limits).map_err(|_| SubmissionStoreError::Capacity)?;
        Ok(Some(RetainedValidatorSubject {
            value,
            candidate: subject.candidate,
            submission_id,
            subject_checksum,
            function_review: subject.function_partner.is_some(),
        }))
    }

    pub async fn read(
        &self,
        id: Uuid,
    ) -> Result<Option<RetainedReviewSubmission>, SubmissionStoreError> {
        if id.is_nil() {
            return Err(SubmissionStoreError::Invalid);
        }
        let mut client = self.pool.get().await.map_err(database)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(database)?;
        let retained = read(&tx, id).await?;
        tx.commit().await.map_err(database)?;
        Ok(retained)
    }
}

async fn require_snapshot(tx: &Transaction<'_>) -> Result<(), SubmissionStoreError> {
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(database)?
        .get(0);
    if !coherent {
        return Err(RevisionStoreError::SnapshotIsolation.into());
    }
    Ok(())
}
async fn read(
    tx: &Transaction<'_>,
    id: Uuid,
) -> Result<Option<RetainedReviewSubmission>, SubmissionStoreError> {
    let Some(row) = tx.query_opt("SELECT actor,component_id,candidate_version,base_version,subject_bytes,checksum FROM reborn_component_review_submissions WHERE submission_id=$1", &[&id]).await.map_err(database)? else { return Ok(None); };
    let bytes: String = row.get(4);
    let value = strict_json(
        &bytes,
        ContractLimits {
            max_depth: 64,
            max_nodes: 262144,
            max_bytes: MAX_SUBJECT_BYTES,
        },
    )
    .map_err(|_| SubmissionStoreError::Invalid)?;
    let subject: Subject =
        serde_json::from_value(value).map_err(|_| SubmissionStoreError::Invalid)?;
    if !matches!(
        subject.format.as_str(),
        "component-review-submission/1"
            | "component-review-submission/2"
            | "component-review-submission/3"
    ) || subject.submission_id != id
    {
        return Err(SubmissionStoreError::Invalid);
    }
    let candidate = ComponentRevisionDraft::from_json(&subject.candidate_bytes)
        .map_err(RevisionStoreError::from)?;
    let base = subject.base.as_ref().map(Reference::parsed).transpose()?;
    let dependencies = subject
        .dependencies
        .iter()
        .map(Reference::parsed)
        .collect::<Result<Vec<_>, _>>()?;
    let draft = match (
        subject.format.as_str(),
        subject.expected_previous_version,
        &subject.function_partner,
    ) {
        ("component-review-submission/1", None, None) => {
            ReviewSubmissionDraft::new(id, &subject.actor, candidate, base, dependencies)?
        }
        ("component-review-submission/2", Some(head), None) => {
            ReviewSubmissionDraft::new_after_head(
                id,
                &subject.actor,
                candidate,
                base,
                dependencies,
                head,
            )?
        }
        ("component-review-submission/3", Some(head), Some(partner)) => {
            ReviewSubmissionDraft::new_function(
                id,
                &subject.actor,
                candidate,
                base,
                dependencies,
                head,
                partner.to_draft()?,
            )?
        }
        _ => return Err(SubmissionStoreError::Invalid),
    };
    let actual = subject.candidate.parsed()?;
    if draft.reference != actual
        || draft.bytes != bytes
        || draft.checksum != row.get::<_, String>(5)
        || draft.actor != row.get::<_, String>(0)
        || actual.uuid != row.get::<_, Uuid>(1)
        || actual.version as i64 != row.get::<_, i64>(2)
        || draft.base.map(|base| base.version as i64) != row.get::<_, Option<i64>>(3)
    {
        return Err(SubmissionStoreError::Invalid);
    }
    verify_request(tx, id, &draft.checksum).await?;
    if draft.function_partner.is_some() {
        if let Some(base) = draft.base {
            PgComponentRevisionStore::read_revision_in_transaction(tx, base).await?;
        }
        if let Some(FunctionPartner::Changed {
            base: Some(base), ..
        }) = &draft.function_partner
        {
            PgComponentRevisionStore::read_revision_in_transaction(tx, base.parsed()?).await?;
        }
    }
    let (roots, references) = subject_references(&draft)?;
    let snapshot =
        PgComponentRevisionStore::read_exact_in_transaction(tx, &roots, &references).await?;
    let function_partner = retained_partner(&draft, &snapshot)?;
    Ok(Some(RetainedReviewSubmission {
        id,
        actor: draft.actor,
        candidate: actual,
        function_partner,
        base: draft.base,
        dependencies: draft.dependencies,
        subject_bytes: bytes,
        checksum: draft.checksum,
        snapshot,
    }))
}

fn subject_references(
    draft: &ReviewSubmissionDraft,
) -> Result<(Vec<Uuid>, Vec<ComponentRevisionRef>), SubmissionStoreError> {
    let mut roots = vec![draft.reference.uuid];
    let mut references = draft.dependencies.clone();
    references.push(draft.reference);
    if let Some(partner) = &draft.function_partner {
        let reference = partner.reference()?;
        roots.push(reference.uuid);
        references.push(reference);
    }
    Ok((roots, references))
}

fn retained_partner(
    draft: &ReviewSubmissionDraft,
    snapshot: &RetainedComponentSnapshot,
) -> Result<Option<RetainedFunctionPartner>, SubmissionStoreError> {
    let Some(partner) = &draft.function_partner else {
        return Ok(None);
    };
    partner.require_pair(draft.reference, snapshot)?;
    let base = match partner {
        FunctionPartner::Changed { base, .. } => {
            base.as_ref().map(Reference::parsed).transpose()?
        }
        FunctionPartner::Context { .. } => None,
    };
    Ok(Some(RetainedFunctionPartner {
        reference: partner.reference()?,
        changed: partner.is_changed(),
        base,
    }))
}

async fn verify_request(
    tx: &Transaction<'_>,
    id: Uuid,
    checksum: &str,
) -> Result<(), SubmissionStoreError> {
    let request = tx
        .query_opt(
            "SELECT subject_checksum FROM reborn_component_review_requests WHERE submission_id=$1",
            &[&id],
        )
        .await
        .map_err(database)?
        .ok_or(SubmissionStoreError::Invalid)?;
    if request.get::<_, String>(0) != checksum {
        return Err(SubmissionStoreError::Invalid);
    }
    Ok(())
}
