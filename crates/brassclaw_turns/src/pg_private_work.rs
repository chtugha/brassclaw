//! Private branch of PgTurnStateStore. The domain owner verifies originals and
//! prepares the existing Monty admission; this owner alone mutates lifecycle.
use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

use super::{PgTurnStateStore, map_pg, map_pg_pool};
use crate::{
    TurnError,
    private_work::{
        ClaimedPrivateWork, PrivateOriginalRef, PrivatePreparationReason, PrivateWorkRequest,
    },
    runner::ClaimRunRequest,
};

fn invalid() -> TurnError {
    TurnError::Unavailable {
        reason: "private work original or claim is invalid".into(),
    }
}
fn checksum(bytes: &str) -> [u8; 32] {
    Sha256::digest(bytes.as_bytes()).into()
}
fn hash(row: &tokio_postgres::Row, column: &str) -> Result<[u8; 32], TurnError> {
    row.try_get::<_, Vec<u8>>(column)
        .map_err(map_pg)?
        .try_into()
        .map_err(|_| invalid())
}
fn read_request(row: &tokio_postgres::Row) -> Result<(PrivateWorkRequest, [u8; 32]), TurnError> {
    let bytes: String = row.try_get("private_request_bytes").map_err(map_pg)?;
    let expected = hash(row, "private_request_checksum")?;
    if checksum(&bytes) != expected {
        return Err(invalid());
    }
    let request: PrivateWorkRequest = serde_json::from_str(&bytes).map_err(|_| invalid())?;
    if request.work_id() != row.try_get::<_, Uuid>("work_id").map_err(map_pg)?
        || request.run_id().as_uuid()
            != row.try_get::<_, Uuid>("execution_run_id").map_err(map_pg)?
        || request.context().tenant_id.as_str()
            != row.try_get::<_, String>("tenant_id").map_err(map_pg)?
    {
        return Err(invalid());
    }
    Ok((request, expected))
}

/// Existing composition/domain owner checks exact original subject/profile and
/// scope inside the same transaction. This port may neither schedule nor commit.
#[async_trait]
pub trait PgPrivateOriginalVerifier: Send + Sync {
    async fn verify_original(
        &self,
        transaction: &Transaction<'_>,
        request: &PrivateWorkRequest,
    ) -> Result<(), TurnError>;
}

/// An existing admission owner retains its address synchronously before the
/// first possible reservation write. A lost COMMIT ACK cannot discard it.
pub trait PgPrivateClaimParticipant: Send + Sync {
    fn retain_reservation(
        &self,
        claimed: &ClaimedPrivateWork,
    ) -> Result<Arc<dyn PgPrivateClaimReservation>, TurnError>;
}
#[async_trait]
pub trait PgPrivateClaimReservation: Send + Sync {
    fn original_claim(&self) -> Result<&ClaimedPrivateWork, TurnError>;
    async fn reserve_in_transaction(&self, transaction: &Transaction<'_>) -> Result<(), TurnError>;
}

impl PgTurnStateStore {
    pub async fn get_private_work_state(
        &self,
        work_id: Uuid,
    ) -> Result<Option<crate::private_work::PrivateWorkState>, TurnError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        Self::private_work_state_on(&**client, &self.tenant_id, work_id).await
    }

    pub async fn get_private_work_state_in_transaction(
        &self,
        tx: &Transaction<'_>,
        work_id: Uuid,
    ) -> Result<Option<crate::private_work::PrivateWorkState>, TurnError> {
        Self::private_work_state_on(tx, &self.tenant_id, work_id).await
    }

    /// Same-owner metadata read for a held transaction; no second pool checkout.
    pub async fn read_private_work_state_in_transaction(
        tx: &Transaction<'_>,
        tenant_id: &str,
        work_id: Uuid,
    ) -> Result<Option<crate::private_work::PrivateWorkState>, TurnError> {
        Self::private_work_state_on(tx, tenant_id, work_id).await
    }

    async fn private_work_state_on<C: tokio_postgres::GenericClient + Sync>(
        client: &C,
        tenant_id: &str,
        work_id: Uuid,
    ) -> Result<Option<crate::private_work::PrivateWorkState>, TurnError> {
        let Some(row) = client
            .query_opt(
                "SELECT * FROM brassclaw_turns
            WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2",
                &[&tenant_id, &work_id],
            )
            .await
            .map_err(map_pg)?
        else {
            return Ok(None);
        };
        let (request, request_checksum) = read_request(&row)?;
        let terminal_checksum = row
            .get::<_, Option<Vec<u8>>>("private_terminal_checksum")
            .map(|bytes| bytes.try_into().map_err(|_| invalid()))
            .transpose()?;
        Ok(Some(crate::private_work::PrivateWorkState {
            request,
            request_checksum,
            status: serde_json::from_value(serde_json::Value::String(row.get("status")))
                .map_err(|_| invalid())?,
            version: u64::try_from(row.get::<_, i64>("version")).map_err(|_| invalid())?,
            cancellation_revision: u64::try_from(row.get::<_, i64>("private_cancel_revision"))
                .map_err(|_| invalid())?,
            terminal_checksum,
        }))
    }

    pub async fn submit_private_work(
        &self,
        request: &PrivateWorkRequest,
        reason: PrivatePreparationReason,
        verifier: &dyn PgPrivateOriginalVerifier,
    ) -> Result<[u8; 32], TurnError> {
        if request.context().tenant_id.as_str() != self.tenant_id {
            return Err(invalid());
        }
        let bytes = serde_json::to_string(request).map_err(|_| invalid())?;
        let digest = checksum(&bytes);
        let (validator, prefix) = match request.original() {
            PrivateOriginalRef::ValidatorStructural { submission_id, .. } => {
                (Some(*submission_id), None)
            }
            PrivateOriginalRef::PrefixSourceOnly { job_id, .. } => (None, Some(*job_id)),
        };
        let reason = match reason {
            PrivatePreparationReason::PreparationPending => "preparation_pending",
            PrivatePreparationReason::QualificationUnavailable => "qualification_unavailable",
        };
        let mut client = self.pool.get().await.map_err(map_pg_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;
        verifier.verify_original(&tx, request).await?;
        tx.execute(
            "INSERT INTO brassclaw_turns(id,tenant_id,turn_id,status,version,record_kind,
            work_id,execution_run_id,private_request_bytes,private_request_checksum,
            validator_submission_id,prefix_job_id,preparation_reason,
            private_claim_generation,private_claim_attempts,private_cancel_revision)
            VALUES($1,$2,NULL,'awaiting_preparation',1,'private_work',$3,$4,$5,$6,$7,$8,$9,0,0,0)
            ON CONFLICT(work_id) DO NOTHING",
            &[
                &format!("private-work:{}", request.work_id()),
                &self.tenant_id,
                &request.work_id(),
                &request.run_id().as_uuid(),
                &bytes,
                &&digest[..],
                &validator,
                &prefix,
                &reason,
            ],
        )
        .await
        .map_err(map_pg)?;
        // Read Committed's next statement sees an identical concurrent insert.
        // A caller-owned RR variant must propagate serialization/invisibility;
        // never interpret a missing row as proof of successful idempotence.
        let row = tx
            .query_opt(
                "SELECT * FROM brassclaw_turns WHERE record_kind='private_work'
            AND tenant_id=$1 AND work_id=$2 FOR UPDATE",
                &[&self.tenant_id, &request.work_id()],
            )
            .await
            .map_err(map_pg)?
            .ok_or_else(invalid)?;
        let (original, stored) = read_request(&row)?;
        if original != *request || stored != digest {
            return Err(invalid());
        }
        tx.commit().await.map_err(map_pg)?;
        Ok(digest)
    }

    /// Composition first retains immutable definition bytes in this transaction.
    /// FK and CAS bind exactly one complete seal; no current-settings lookup here.
    pub async fn seal_private_work_in_transaction(
        &self,
        tx: &Transaction<'_>,
        work_id: Uuid,
        expected_version: i64,
        request_checksum: [u8; 32],
        definition_checksum: [u8; 32],
    ) -> Result<(), TurnError> {
        if expected_version <= 0 {
            return Err(invalid());
        }
        let changed=tx.execute("UPDATE brassclaw_turns SET status='ready',
            private_definition_checksum=$5,preparation_reason=NULL,ready_at=clock_timestamp(),version=version+1
            WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2
            AND version=$3 AND private_request_checksum=$4 AND status='awaiting_preparation'
            AND private_cancel_revision=0 AND private_definition_checksum IS NULL",&[
                &self.tenant_id,&work_id,&expected_version,&&request_checksum[..],&&definition_checksum[..],
            ]).await.map_err(map_pg)?;
        if changed != 1 {
            return Err(invalid());
        }
        Ok(())
    }

    /// Reserved admission and the real Workclaim commit together. A returned
    /// claim is never assembled from a test fixture, JSON flag or chat snapshot.
    pub async fn claim_next_private_work(
        &self,
        request: &ClaimRunRequest,
        participant: &dyn PgPrivateClaimParticipant,
    ) -> Result<Option<ClaimedPrivateWork>, TurnError> {
        if request.scope_filter.is_some() {
            return Ok(None);
        }
        let ttl = self
            .limits
            .runner_lease_ttl
            .num_microseconds()
            .filter(|v| *v > 0)
            .ok_or_else(invalid)?;
        let mut client = self.pool.get().await.map_err(map_pg_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;
        let Some(row) = tx
            .query_opt(
                "SELECT * FROM brassclaw_turns
            WHERE record_kind='private_work' AND tenant_id=$1 AND status='ready'
            AND private_cancel_revision=0 ORDER BY ready_at,work_id FOR UPDATE SKIP LOCKED LIMIT 1",
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?
        else {
            tx.commit().await.map_err(map_pg)?;
            return Ok(None);
        };
        let (original, request_checksum) = read_request(&row)?;
        let claimed = ClaimedPrivateWork {
            request: original,
            request_checksum,
            definition_checksum: hash(&row, "private_definition_checksum")?,
            runner_id: request.runner_id,
            lease_token: request.lease_token,
            generation: 1,
            cancellation_revision: 0,
        };
        // Retention precedes both writes and the ambiguous commit boundary.
        let reservation = participant.retain_reservation(&claimed)?;
        // A known still-Ready original may reuse its pre-COMMIT native address.
        // A committed claim can never return to Ready; pre-I/O submission intent
        // separately prevents any potentially submitted attempt being replaced.
        let original = reservation.original_claim()?;
        if original.request() != claimed.request()
            || original.request_checksum() != claimed.request_checksum()
            || original.definition_checksum() != claimed.definition_checksum()
            || original.attempt().runner_id != request.runner_id
        {
            return Err(invalid());
        }
        let claimed = original.clone();
        let lease = serde_json::to_vec(&claimed.attempt().lease_token).map_err(|_| invalid())?;
        let claim_checksum: [u8; 32] = Sha256::digest(lease).into();
        let changed=tx.execute("UPDATE brassclaw_turns SET status='claimed',version=version+1,
            private_runner_id=$3,private_claim_checksum=$4,private_claim_generation=1,
            private_claim_attempts=1,private_lease_deadline=clock_timestamp()+$5::bigint*interval '1 microsecond',
            private_admission_run=execution_run_id
            WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2 AND status='ready'",
            &[&self.tenant_id,&claimed.request.work_id(),&request.runner_id.as_uuid(),&&claim_checksum[..],&ttl])
            .await.map_err(map_pg)?;
        if changed != 1 {
            return Err(invalid());
        }
        reservation.reserve_in_transaction(&tx).await?;
        tx.commit().await.map_err(map_pg)?;
        Ok(Some(claimed))
    }

    pub async fn heartbeat_private_work(
        &self,
        claimed: &ClaimedPrivateWork,
    ) -> Result<bool, TurnError> {
        let ttl = self
            .limits
            .runner_lease_ttl
            .num_microseconds()
            .filter(|v| *v > 0)
            .ok_or_else(invalid)?;
        let lease = serde_json::to_vec(&claimed.lease_token).map_err(|_| invalid())?;
        let digest: [u8; 32] = Sha256::digest(lease).into();
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let row = client
            .query_opt(
                "UPDATE brassclaw_turns SET version=version+1,
            private_lease_deadline=clock_timestamp()+$5::bigint*interval '1 microsecond'
            WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2
            AND status='claimed' AND private_runner_id=$3 AND private_claim_checksum=$4
            AND private_claim_generation=1 AND private_lease_deadline>clock_timestamp()
            RETURNING private_cancel_revision",
                &[
                    &self.tenant_id,
                    &claimed.request.work_id(),
                    &claimed.runner_id.as_uuid(),
                    &&digest[..],
                    &ttl,
                ],
            )
            .await
            .map_err(map_pg)?
            .ok_or_else(invalid)?;
        Ok(row.get::<_, i64>(0) > 0)
    }

    pub async fn cancel_private_work(&self, work_id: Uuid) -> Result<(), TurnError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let changed=client.execute("UPDATE brassclaw_turns SET version=version+1,
            status=CASE WHEN status IN ('ready','awaiting_preparation') THEN 'cancelled' ELSE status END,
            private_cancel_revision=private_cancel_revision+1,private_cancel_requested_at=clock_timestamp()
            WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2
            AND status IN ('awaiting_preparation','ready','claimed') AND private_cancel_revision=0",
            &[&self.tenant_id,&work_id]).await.map_err(map_pg)?;
        if changed == 0 {
            let exists: bool = client
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM brassclaw_turns
                WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2
                AND (private_cancel_revision>0 OR status='settled'))",
                    &[&self.tenant_id, &work_id],
                )
                .await
                .map_err(map_pg)?
                .get(0);
            if !exists {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub async fn fence_expired_private_work(&self) -> Result<(), TurnError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        client
            .execute(
                "UPDATE brassclaw_turns SET status='reconciliation_required',version=version+1
            WHERE record_kind='private_work' AND tenant_id=$1 AND status='claimed'
            AND private_lease_deadline<=clock_timestamp()",
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }
}

impl PgTurnStateStore {
    pub(super) async fn mark_private_work_unresolved(
        &self,
        claimed: &ClaimedPrivateWork,
    ) -> Result<(), TurnError> {
        let lease = serde_json::to_vec(&claimed.attempt().lease_token).map_err(|_| invalid())?;
        let digest: [u8; 32] = Sha256::digest(lease).into();
        let mut client = self.pool.get().await.map_err(map_pg_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;
        let row=tx.query_opt("SELECT status,private_claim_checksum,private_runner_id
            FROM brassclaw_turns WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2 FOR UPDATE",
            &[&self.tenant_id,&claimed.request().work_id()]).await.map_err(map_pg)?.ok_or_else(invalid)?;
        if row.get::<_, Vec<u8>>(1).as_slice() != digest
            || row.get::<_, Uuid>(2) != claimed.attempt().runner_id.as_uuid()
        {
            return Err(invalid());
        }
        if row.get::<_, String>(0) == "claimed" {
            tx.execute(
                "UPDATE brassclaw_turns SET status='reconciliation_required',version=version+1
                WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2",
                &[&self.tenant_id, &claimed.request().work_id()],
            )
            .await
            .map_err(map_pg)?;
        } else if !matches!(row.get::<_, &str>(0), "reconciliation_required" | "settled") {
            return Err(invalid());
        }
        tx.commit().await.map_err(map_pg)
    }
}
