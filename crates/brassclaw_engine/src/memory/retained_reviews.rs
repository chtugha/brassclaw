//! Bind IBS's exact usage selections to immutable review records in the
//! caller-owned catalogue snapshot. This does not select latest, activate a
//! component, establish trusted producer provenance or authorize execution.
//! Draft behavioral validation deliberately uses a separate unapproved path.

use std::collections::{BTreeMap, BTreeSet};

use brassclaw_pg::PgError;
use brassclaw_skills::association_review_store::{
    AssociationReviewStoreError, RetainedAssociationReviewRecords,
    retain_authored_association_reviews,
};
use tokio_postgres::Transaction;
use uuid::Uuid;

use super::retained_tools::RetainedToolProgram;

#[derive(thiserror::Error)]
pub enum RetainedReviewError {
    #[error("retained usage review selection is incomplete or contains unrelated records")]
    Selection,
    #[error("retained usage reviews require one repeatable-read or serializable catalogue view")]
    Isolation,
    #[error("retained usage review database operation failed")]
    Database(#[source] PgError),
    #[error("retained usage review record read failed")]
    Records(#[source] AssociationReviewStoreError),
}
impl std::fmt::Debug for RetainedReviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_string())
    }
}

/// Retains one exact approval/evidence/usage closure per selected Skill, shared
/// across its repeated uses. Recipe/pure-logic approval, producer provenance,
/// catalogue activation and actual Tool artifact/ABI checks remain separate.
pub struct RetainedToolReviewRecords {
    usages: BTreeMap<Uuid, RetainedAssociationReviewRecords>,
}
impl RetainedToolReviewRecords {
    pub fn usages(&self) -> &BTreeMap<Uuid, RetainedAssociationReviewRecords> {
        &self.usages
    }
    pub fn approval_references(&self) -> BTreeMap<Uuid, Uuid> {
        self.usages
            .iter()
            .map(|(skill, records)| (*skill, records.approval().approval_id()))
            .collect()
    }
}

/// Catalogue selection supplies exact immutable approval IDs, keyed by each
/// selected usage's stable Skill identity. A binding/manifest/individual legacy
/// component approval cannot supply the missing combination record. Errors
/// remain explicit and must never cause No-Match or a Tier-2 replay.
pub async fn retain_prepared_tool_reviews(
    tx: &Transaction<'_>,
    program: &RetainedToolProgram,
    approval_ids: &BTreeMap<Uuid, Uuid>,
) -> Result<RetainedToolReviewRecords, RetainedReviewError> {
    let selected: BTreeSet<_> = program
        .bindings()
        .values()
        .map(|binding| binding.skill().uuid)
        .collect();
    if selected != approval_ids.keys().copied().collect()
        || approval_ids.values().any(Uuid::is_nil)
        || approval_ids
            .values()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != approval_ids.len()
    {
        return Err(RetainedReviewError::Selection);
    }
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(|error| RetainedReviewError::Database(error.into()))?
        .get(0);
    if !coherent {
        return Err(RetainedReviewError::Isolation);
    }
    let mut usages = BTreeMap::new();
    for binding in program.bindings().values() {
        let skill = binding.skill().uuid;
        if usages.contains_key(&skill) {
            continue;
        }
        let records = retain_authored_association_reviews(
            tx,
            approval_ids[&skill],
            binding.association(),
            binding.combination(),
        )
        .await
        .map_err(RetainedReviewError::Records)?;
        usages.insert(skill, records);
    }
    Ok(RetainedToolReviewRecords { usages })
}
