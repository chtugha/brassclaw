//! Metadata custody for the original single-case native operation. One write,
//! then one fresh coherent readback; no Tool, VM or task replay occurs here.
use std::sync::Arc;

use brassclaw_pg::PgPool;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::{pg_transaction_client::TransactionClient, private_work_definition::checksum};

/// Scoped metadata failures retain their actual private source. Public/debug
/// rendering exposes only the stage and classification, never SQL diagnostics.
pub(crate) struct EvidenceError {
    phase: &'static str,
    kind: &'static str,
    sqlstate: Option<String>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    secondary: Option<Box<Self>>,
}
impl EvidenceError {
    fn source(phase: &'static str, error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            phase,
            kind: "dependency",
            sqlstate: None,
            source: Some(Box::new(error)),
            secondary: None,
        }
    }
    fn database(phase: &'static str, error: tokio_postgres::Error) -> Self {
        Self {
            phase,
            kind: if error.is_closed() {
                "connection"
            } else if error.code().is_some() {
                "database"
            } else {
                "transport"
            },
            sqlstate: error.code().map(|code| code.code().to_owned()),
            source: Some(Box::new(error)),
            secondary: None,
        }
    }
    fn classified(phase: &'static str, kind: &'static str) -> Self {
        Self {
            phase,
            kind,
            sqlstate: None,
            source: None,
            secondary: None,
        }
    }
    pub(crate) fn facts(&self) -> serde_json::Value {
        serde_json::json!({"phase":self.phase,"kind":self.kind,"sqlstate":self.sqlstate,
            "secondary":self.secondary.as_ref().map(|error|error.facts())})
    }
}
impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "original Pure case metadata {} failed ({})",
            self.phase, self.kind
        )
    }
}
impl std::fmt::Debug for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for EvidenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

pub(crate) struct EvidenceStore {
    pub(crate) pool: Arc<PgPool>,
    pub(crate) storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
}
impl EvidenceStore {
    /// A missing acknowledgement of a NEW execution never permits starting its
    /// VM, even if its intent can subsequently be read. The retained case stays
    /// fenced for reconciliation under the same ID.
    pub(crate) async fn begin(
        &self,
        case_id: Uuid,
        invocation_id: Uuid,
        bytes: &str,
    ) -> Result<[u8; 32], EvidenceError> {
        let digest = checksum(bytes);
        let timeout = self
            .storage
            .snapshot()
            .map_err(|error| EvidenceError::classified("settings", error))?
            .validate()
            .map_err(|error| EvidenceError::classified("settings", error))?
            .timeout;
        let write =
            tokio::time::timeout(timeout, async {
                let mut client =
                    TransactionClient::from_client(self.pool.get().await.map_err(|error| {
                        EvidenceError::source("intent_write_connection", error)
                    })?);
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .start()
                    .await
                    .map_err(|error| EvidenceError::database("intent_write", error))?;
                let changed = tx
                    .execute(
                        "INSERT INTO reborn_original_pure_case_executions(
                case_id,work_id,run_id,invocation_id,execution_bytes,checksum)
                VALUES($1,$1,$1,$2,$3,$4) ON CONFLICT(case_id) DO NOTHING",
                        &[&case_id, &invocation_id, &bytes, &&digest[..]],
                    )
                    .await
                    .map_err(|error| EvidenceError::database("intent_write", error))?;
                if changed != 1 {
                    return Err(EvidenceError::classified("intent_write", "already_started"));
                }
                tx.commit()
                    .await
                    .map_err(|error| EvidenceError::database("intent_write_commit", error))?;
                client.recycle_after_commit();
                Ok(())
            })
            .await
            .map_err(|_| EvidenceError::classified("intent_write", "deadline"))
            .and_then(|result| result);
        let read = tokio::time::timeout(
            timeout,
            self.require_execution(case_id, invocation_id, bytes, digest),
        )
        .await
        .map_err(|_| EvidenceError::classified("intent_readback", "deadline"))
        .and_then(|result| result);
        if let Err(mut error) = write {
            error.secondary = read.err().map(Box::new);
            return Err(error);
        }
        read?;
        Ok(digest)
    }
    async fn require_execution(
        &self,
        case_id: Uuid,
        invocation_id: Uuid,
        bytes: &str,
        digest: [u8; 32],
    ) -> Result<(), EvidenceError> {
        let mut client = TransactionClient::from_client(
            self.pool
                .get()
                .await
                .map_err(|error| EvidenceError::source("intent_readback_connection", error))?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|error| EvidenceError::database("intent_readback", error))?;
        let row = tx
            .query_opt(
                "SELECT work_id,run_id,invocation_id,execution_bytes,checksum
            FROM reborn_original_pure_case_executions WHERE case_id=$1",
                &[&case_id],
            )
            .await
            .map_err(|error| EvidenceError::database("intent_readback", error))?
            .ok_or_else(|| EvidenceError::classified("intent_readback", "missing_original"))?;
        if row.get::<_, Uuid>(0) != case_id
            || row.get::<_, Uuid>(1) != case_id
            || row.get::<_, Uuid>(2) != invocation_id
            || row.get::<_, &str>(3) != bytes
            || row.get::<_, Vec<u8>>(4).as_slice() != digest
        {
            return Err(EvidenceError::classified(
                "intent_readback",
                "exact_mismatch",
            ));
        }
        tx.commit()
            .await
            .map_err(|error| EvidenceError::database("intent_readback_commit", error))?;
        client.recycle_after_commit();
        Ok(())
    }

    /// Complete actual receipt bytes remain owned by the native operation
    /// before this await. A lost metadata ACK is resolved only by exact readback.
    pub(crate) async fn observe(
        &self,
        case_id: Uuid,
        execution: [u8; 32],
        observation: &str,
        native: &str,
        originals: &std::collections::BTreeMap<
            &'static str,
            Arc<brassclaw_engine::executor::retained_pure_invocation::NativePureReceipt>,
        >,
    ) -> Result<[u8; 32], EvidenceError> {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(observation.as_bytes());
        hash.update(native.as_bytes());
        let digest: [u8; 32] = hash.finalize().into();
        let timeout = self
            .storage
            .snapshot()
            .map_err(|error| EvidenceError::classified("settings", error))?
            .validate()
            .map_err(|error| EvidenceError::classified("settings", error))?
            .timeout;
        let write = tokio::time::timeout(timeout, async {
            let mut client =
                TransactionClient::from_client(self.pool.get().await.map_err(|error| EvidenceError::source("observation_write_connection",error))?);
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .map_err(|error| EvidenceError::database("observation_write",error))?;
            for (stage,receipt) in originals {
                let ordinal=i64::try_from(receipt.ordinal()).map_err(|error| EvidenceError::source("receipt_ordinal",error))?;
                tx.execute("INSERT INTO reborn_original_pure_case_native_receipts(case_id,stage,ordinal,
                    execution_checksum,request_id,receipt_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6,$7)
                    ON CONFLICT(case_id,stage) DO NOTHING", &[&case_id,stage,&ordinal,&&execution[..],
                    &receipt.request_id(),&receipt.bytes(),&&receipt.checksum()[..]]).await.map_err(|error| EvidenceError::database("observation_write",error))?;
            }
            tx.execute(
                "INSERT INTO reborn_original_pure_case_observations(case_id,execution_checksum,
                observation_bytes,native_receipts_bytes,checksum) VALUES($1,$2,$3,$4,$5)
                ON CONFLICT(case_id) DO NOTHING",
                &[
                    &case_id,
                    &&execution[..],
                    &observation,
                    &native,
                    &&digest[..],
                ],
            )
            .await
            .map_err(|error| EvidenceError::database("observation_write",error))?;
            tx.commit().await.map_err(|error| EvidenceError::database("observation_write_commit",error))?;
            client.recycle_after_commit();
            Ok(())
        })
        .await
        .map_err(|_| EvidenceError::classified("observation_write", "deadline"))
        .and_then(|result| result);
        let read = tokio::time::timeout(timeout, async {
            let mut client =
                TransactionClient::from_client(self.pool.get().await.map_err(|error| {
                    EvidenceError::source("observation_readback_connection", error)
                })?);
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(|error| EvidenceError::database("observation_readback", error))?;
            let row = tx
                .query_opt(
                    "SELECT execution_checksum,observation_bytes,native_receipts_bytes,checksum
                FROM reborn_original_pure_case_observations WHERE case_id=$1",
                    &[&case_id],
                )
                .await
                .map_err(|error| EvidenceError::database("observation_readback", error))?
                .ok_or_else(|| {
                    EvidenceError::classified("observation_readback", "missing_original")
                })?;
            if row.get::<_, Vec<u8>>(0).as_slice() != execution
                || row.get::<_, &str>(1) != observation
                || row.get::<_, &str>(2) != native
                || row.get::<_, Vec<u8>>(3).as_slice() != digest
            {
                return Err(EvidenceError::classified(
                    "observation_readback",
                    "exact_mismatch",
                ));
            }
            let rows = tx
                .query(
                    "SELECT stage,ordinal,execution_checksum,request_id,receipt_bytes,checksum
                FROM reborn_original_pure_case_native_receipts WHERE case_id=$1",
                    &[&case_id],
                )
                .await
                .map_err(|error| EvidenceError::database("observation_readback", error))?;
            if rows.len() != originals.len() {
                return Err(EvidenceError::classified(
                    "observation_readback",
                    "exact_mismatch",
                ));
            }
            for row in rows {
                let receipt = originals.get(row.get::<_, &str>(0)).ok_or_else(|| {
                    EvidenceError::classified("observation_readback", "exact_mismatch")
                })?;
                if row.get::<_, i64>(1)
                    != i64::try_from(receipt.ordinal())
                        .map_err(|error| EvidenceError::source("receipt_ordinal", error))?
                    || row.get::<_, Vec<u8>>(2).as_slice() != execution
                    || row.get::<_, Uuid>(3) != receipt.request_id()
                    || row.get::<_, &str>(4) != receipt.bytes()
                    || row.get::<_, Vec<u8>>(5).as_slice() != receipt.checksum()
                {
                    return Err(EvidenceError::classified(
                        "observation_readback",
                        "exact_mismatch",
                    ));
                }
            }
            tx.commit()
                .await
                .map_err(|error| EvidenceError::database("observation_readback_commit", error))?;
            client.recycle_after_commit();
            Ok(())
        })
        .await
        .map_err(|_| EvidenceError::classified("observation_readback", "deadline"))
        .and_then(|result| result);
        match read {
            Ok(()) => Ok(digest),
            Err(error) => {
                if let Err(mut write_error) = write {
                    write_error.secondary = Some(Box::new(error));
                    Err(write_error)
                } else {
                    Err(error)
                }
            }
        }
    }
    /// An immutable technical failure is evidence of an interrupted case, never
    /// a candidate outcome or a successful Harness delivery. Retrying this exact
    /// metadata write cannot submit any worker command.
    pub(crate) async fn technical_failure(
        &self,
        case_id: Uuid,
        invocation_id: Uuid,
        phase: &'static str,
        bytes: &str,
    ) -> Result<(), EvidenceError> {
        let digest = checksum(bytes);
        let timeout = self
            .storage
            .snapshot()
            .map_err(|error| EvidenceError::classified("settings", error))?
            .validate()
            .map_err(|error| EvidenceError::classified("settings", error))?
            .timeout;
        let write = tokio::time::timeout(timeout, async {
            let mut client=TransactionClient::from_client(self.pool.get().await
                .map_err(|error|EvidenceError::source("failure_write_connection",error))?);
            let tx=client.build_transaction().isolation_level(IsolationLevel::RepeatableRead).start().await
                .map_err(|error|EvidenceError::database("failure_write",error))?;
            tx.execute("INSERT INTO reborn_original_pure_case_failures(case_id,invocation_id,phase,failure_bytes,checksum)
                VALUES($1,$2,$3,$4,$5) ON CONFLICT(case_id,phase) DO NOTHING",
                &[&case_id,&invocation_id,&phase,&bytes,&&digest[..]]).await
                .map_err(|error|EvidenceError::database("failure_write",error))?;
            tx.commit().await.map_err(|error|EvidenceError::database("failure_write_commit",error))?;
            client.recycle_after_commit();
            Ok::<(),EvidenceError>(())
        }).await.map_err(|_|EvidenceError::classified("failure_write","deadline")).and_then(|result|result);
        let read=tokio::time::timeout(timeout, async {
            let mut client=TransactionClient::from_client(self.pool.get().await
                .map_err(|error|EvidenceError::source("failure_readback_connection",error))?);
            let tx=client.build_transaction().isolation_level(IsolationLevel::RepeatableRead).read_only(true).start().await
                .map_err(|error|EvidenceError::database("failure_readback",error))?;
            let row=tx.query_opt("SELECT invocation_id,failure_bytes,checksum FROM reborn_original_pure_case_failures
                WHERE case_id=$1 AND phase=$2", &[&case_id,&phase]).await
                .map_err(|error|EvidenceError::database("failure_readback",error))?.ok_or_else(||EvidenceError::classified("failure_readback","missing_original"))?;
            if row.get::<_,Uuid>(0)!=invocation_id || row.get::<_,&str>(1)!=bytes || row.get::<_,Vec<u8>>(2).as_slice()!=digest {
                return Err(EvidenceError::classified("failure_readback","exact_mismatch"));
            }
            tx.commit().await.map_err(|error|EvidenceError::database("failure_readback_commit",error))?;
            client.recycle_after_commit();
            Ok(())
        }).await.map_err(|_|EvidenceError::classified("failure_readback","deadline")).and_then(|result|result);
        match read {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Err(mut write_error) = write {
                    write_error.secondary = Some(Box::new(error));
                    Err(write_error)
                } else {
                    Err(error)
                }
            }
        }
    }
}
