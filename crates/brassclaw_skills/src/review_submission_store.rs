//! Immutable authoring subjects. Retention supplies neither review evidence nor
//! activation: a validator Recipe must inspect this exact graph separately.

use std::{collections::BTreeSet, sync::Arc};

use brassclaw_pg::{PgError, PgPool};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
    value_contract::{ContractLimits, strict_json},
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
}
impl ReviewSubmissionDraft {
    pub fn new(
        id: Uuid,
        actor: &str,
        candidate: ComponentRevisionDraft,
        base: Option<ComponentRevisionRef>,
        mut dependencies: Vec<ComponentRevisionRef>,
    ) -> Result<Self, SubmissionStoreError> {
        if id.is_nil() || actor.trim().is_empty() || actor.len() > 2048 {
            return Err(SubmissionStoreError::Invalid);
        }
        if dependencies.len() > MAX_DEPENDENCIES {
            return Err(SubmissionStoreError::Capacity);
        }
        if let Some(base) = base {
            Reference::from(base).parsed()?;
            if base.uuid != candidate.uuid() || base.class_code != candidate.class_code() {
                return Err(SubmissionStoreError::Invalid);
            }
        }
        let version = base
            .map_or(0, |base| base.version)
            .checked_add(1)
            .filter(|version| *version <= i64::MAX as u64)
            .ok_or(SubmissionStoreError::Invalid)?;
        let reference = ComponentRevisionRef {
            uuid: candidate.uuid(),
            class_code: candidate.class_code(),
            version,
            checksum: candidate.checksum(),
        };
        let mut unique = BTreeSet::new();
        for dependency in &dependencies {
            Reference::from(*dependency).parsed()?;
            if dependency.uuid == reference.uuid || !unique.insert(dependency.uuid) {
                return Err(SubmissionStoreError::Invalid);
            }
        }
        dependencies.sort();
        let subject = Subject {
            format: "component-review-submission/1".into(),
            submission_id: id,
            actor: actor.into(),
            candidate: reference.into(),
            candidate_bytes: candidate.exact_bytes().into(),
            base: base.map(Into::into),
            dependencies: dependencies.iter().copied().map(Into::into).collect(),
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
        })
    }
}

pub struct RetainedReviewSubmission {
    pub id: Uuid,
    pub actor: String,
    pub candidate: ComponentRevisionRef,
    pub base: Option<ComponentRevisionRef>,
    pub dependencies: Vec<ComponentRevisionRef>,
    pub subject_bytes: String,
    pub checksum: String,
    pub snapshot: RetainedComponentSnapshot,
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
        if let Some(base) = draft.base {
            let version = base.version as i64;
            let row = tx.query_opt("SELECT class_code,checksum FROM reborn_component_revisions WHERE component_id=$1 AND version=$2", &[&base.uuid, &version]).await.map_err(database)?
                .ok_or(SubmissionStoreError::Conflict)?;
            if i32::from(row.get::<_, i16>(0)) != base.class_code
                || row.get::<_, String>(1) != hex(base.checksum)
            {
                return Err(SubmissionStoreError::Conflict);
            }
        }
        let staged = match retention {
            CandidateRetention::Stage => {
                PgComponentRevisionStore::stage_in_transaction(
                    tx,
                    &draft.candidate,
                    draft.base.map_or(0, |base| base.version),
                )
                .await?
            }
            CandidateRetention::Existing => draft.reference,
        };
        if staged != draft.reference {
            return Err(SubmissionStoreError::Invalid);
        }
        let mut references = draft.dependencies.clone();
        references.push(staged);
        let snapshot =
            PgComponentRevisionStore::read_exact_in_transaction(tx, &[staged.uuid], &references)
                .await?;
        let version = staged.version as i64;
        let base_version = draft.base.map(|base| base.version as i64);
        tx.execute("INSERT INTO reborn_component_review_submissions (submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum) VALUES ($1,$2,$3,$4,$5,$6,$7)",
            &[&draft.id, &draft.actor, &staged.uuid, &version, &base_version, &draft.bytes, &draft.checksum]).await.map_err(database)?;
        Ok(RetainedReviewSubmission {
            id: draft.id,
            actor: draft.actor.clone(),
            candidate: staged,
            base: draft.base,
            dependencies: draft.dependencies.clone(),
            subject_bytes: draft.bytes.clone(),
            checksum: draft.checksum.clone(),
            snapshot,
        })
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
    if subject.format != "component-review-submission/1" || subject.submission_id != id {
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
    let draft = ReviewSubmissionDraft::new(id, &subject.actor, candidate, base, dependencies)?;
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
    let mut references = draft.dependencies.clone();
    references.push(actual);
    let snapshot =
        PgComponentRevisionStore::read_exact_in_transaction(tx, &[actual.uuid], &references)
            .await?;
    Ok(Some(RetainedReviewSubmission {
        id,
        actor: draft.actor,
        candidate: actual,
        base: draft.base,
        dependencies: draft.dependencies,
        subject_bytes: bytes,
        checksum: draft.checksum,
        snapshot,
    }))
}
