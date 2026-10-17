//! Shared immutable proof storage, not a validation oracle or an approval grant.
//! Each closed installation owner must independently verify its exact packaged
//! subject and actual durable behavior before constructing its qualified type.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

pub(super) fn checksum(bytes: &str) -> String {
    hex::encode(Sha256::digest(bytes.as_bytes()))
}

pub(super) async fn require_coherent(tx: &Transaction<'_>) -> Result<(), RebornBuildError> {
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(invalid)?
        .get(0);
    if !coherent {
        return Err(invalid(
            "bootstrap approval requires coherent database view",
        ));
    }
    Ok(())
}

pub(super) async fn read_evidence(
    tx: &Transaction<'_>,
    id: Uuid,
) -> Result<Option<String>, RebornBuildError> {
    read(
        tx,
        "SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",
        id,
    )
    .await
}

pub(super) async fn read_association(
    tx: &Transaction<'_>,
    id: Uuid,
) -> Result<Option<String>, RebornBuildError> {
    read(tx, "SELECT approval_bytes,checksum FROM reborn_skill_association_approvals WHERE approval_id=$1", id).await
}

async fn read(
    tx: &Transaction<'_>,
    query: &str,
    id: Uuid,
) -> Result<Option<String>, RebornBuildError> {
    tx.query_opt(query, &[&id])
        .await
        .map_err(invalid)?
        .map(|row| {
            let bytes: String = row.get(0);
            if checksum(&bytes) != row.get::<_, String>(1) {
                Err(invalid("bootstrap proof record integrity failed"))
            } else {
                Ok(bytes)
            }
        })
        .transpose()
}

/// The owner commits this exact Q1 intent before any behavioral effect. False
/// means another/incomplete producer already retained the identity: no replay.
pub(super) async fn retain_intent(
    tx: &Transaction<'_>,
    id: Uuid,
    bytes: &str,
) -> Result<bool, RebornBuildError> {
    Ok(tx.execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING",
        &[&id,&bytes,&checksum(bytes)]).await.map_err(invalid)? == 1)
}

/// Called within the owner's one transaction after real observed behavior.
/// Immutable conflicts fail; this helper never fabricates or approves a report.
pub(super) async fn retain_completed(
    tx: &Transaction<'_>,
    behavior_id: Uuid,
    behavior: &str,
    approval_id: Uuid,
    approval: &str,
) -> Result<(), RebornBuildError> {
    tx.execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3)",
        &[&behavior_id,&behavior,&checksum(behavior)]).await.map_err(invalid)?;
    tx.execute("INSERT INTO reborn_skill_association_approvals(approval_id,approval_bytes,checksum) VALUES($1,$2,$3)",
        &[&approval_id,&approval,&checksum(approval)]).await.map_err(invalid)?;
    Ok(())
}
