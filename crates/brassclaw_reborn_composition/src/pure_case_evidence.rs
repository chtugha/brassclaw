//! Metadata custody for the original single-case native operation. One write,
//! then one fresh coherent readback; no Tool, VM or task replay occurs here.
use std::sync::Arc;

use brassclaw_pg::PgPool;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::{pg_transaction_client::TransactionClient, private_work_definition::checksum};

pub(crate) const EXECUTION_INSERT: &str = "INSERT INTO reborn_original_pure_case_executions(
                case_id,work_id,run_id,invocation_id,execution_bytes,checksum)
                VALUES($1,$1,$1,$2,$3,$4) ON CONFLICT(case_id) DO NOTHING";
pub(crate) const RECEIPT_INSERT: &str = "INSERT INTO reborn_original_pure_case_native_receipts(case_id,stage,ordinal,
                    execution_checksum,request_id,receipt_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6,$7)
                    ON CONFLICT(case_id,stage) DO NOTHING";
pub(crate) const OBSERVATION_INSERT: &str =
    "INSERT INTO reborn_original_pure_case_observations(case_id,execution_checksum,
                observation_bytes,native_receipts_bytes,checksum) VALUES($1,$2,$3,$4,$5)
                ON CONFLICT(case_id) DO NOTHING";
pub(crate) const FAILURE_INSERT: &str = "INSERT INTO reborn_original_pure_case_failures(case_id,invocation_id,phase,failure_bytes,checksum)
                VALUES($1,$2,$3,$4,$5) ON CONFLICT(case_id,phase) DO NOTHING";

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

/// Complete metadata encoding under the original invocation's data policy and
/// the existing PG owner's intrinsic representation. Neither is a new quota.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CaseEncodingError {
    #[error("original case evidence exceeds its captured data policy")]
    PolicyCapacity,
    #[error("original case evidence exceeds PostgreSQL representation")]
    Representation,
    #[error("original case evidence allocation failed")]
    Allocation(#[source] std::collections::TryReserveError),
    #[error("original case evidence JSON encoding failed")]
    Encoding(#[source] brassclaw_common::json::JsonWriteError),
}

pub(crate) fn copy_original_text(
    text: &str,
    original_max_bytes: u64,
) -> Result<String, CaseEncodingError> {
    let bytes = u64::try_from(text.len()).map_err(|_| CaseEncodingError::Representation)?;
    if bytes > original_max_bytes {
        return Err(CaseEncodingError::PolicyCapacity);
    }
    if text.len() > brassclaw_pg::wire_capacity::maximum_text_content_bytes() {
        return Err(CaseEncodingError::Representation);
    }
    let mut copied = String::new();
    copied
        .try_reserve_exact(text.len())
        .map_err(CaseEncodingError::Allocation)?;
    copied.push_str(text);
    Ok(copied)
}

pub(crate) fn encode_original<T: serde::Serialize + ?Sized>(
    value: &T,
    original_max_bytes: u64,
) -> Result<String, CaseEncodingError> {
    encode_original_for(value, original_max_bytes, "", &[])
}

pub(crate) fn encode_original_for<T: serde::Serialize + ?Sized>(
    value: &T,
    original_max_bytes: u64,
    sql: &str,
    other_parameters: &[usize],
) -> Result<String, CaseEncodingError> {
    use brassclaw_common::json::{JsonWriteError, count_json_bytes, write_json};
    let representation = brassclaw_pg::wire_capacity::maximum_text_content_bytes() as u64;
    let limit = original_max_bytes.min(representation);
    let count = count_json_bytes(value, Some(limit)).map_err(|error| match error {
        JsonWriteError::LimitExceeded if original_max_bytes <= representation => {
            CaseEncodingError::PolicyCapacity
        }
        JsonWriteError::LimitExceeded => CaseEncodingError::Representation,
        other => CaseEncodingError::Encoding(other),
    })?;
    let count = usize::try_from(count).map_err(|_| CaseEncodingError::Representation)?;
    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(
            other_parameters
                .len()
                .checked_add(1)
                .ok_or(CaseEncodingError::Representation)?,
        )
        .map_err(CaseEncodingError::Allocation)?;
    parameters.extend_from_slice(other_parameters);
    parameters.push(count);
    if !brassclaw_pg::wire_capacity::text_record_fits(sql.len(), &parameters) {
        return Err(CaseEncodingError::Representation);
    }
    struct Reserved {
        bytes: Vec<u8>,
        maximum: usize,
    }
    impl std::io::Write for Reserved {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self
                .bytes
                .len()
                .checked_add(bytes.len())
                .filter(|n| *n <= self.maximum && *n <= self.bytes.capacity())
                .ok_or_else(|| std::io::Error::other("original case counted JSON size changed"))?;
            self.bytes.extend_from_slice(bytes);
            debug_assert_eq!(self.bytes.len(), next);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Reserved {
        bytes: Vec::new(),
        maximum: count,
    };
    output
        .bytes
        .try_reserve_exact(count)
        .map_err(CaseEncodingError::Allocation)?;
    write_json(&mut output, value).map_err(CaseEncodingError::Encoding)?;
    if output.bytes.len() != count {
        return Err(CaseEncodingError::Encoding(JsonWriteError::Encoding));
    }
    String::from_utf8(output.bytes)
        .map_err(|_| CaseEncodingError::Encoding(JsonWriteError::Encoding))
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
        if !brassclaw_pg::wire_capacity::text_record_fits(
            EXECUTION_INSERT.len(),
            &[16, 16, bytes.len(), 32],
        ) {
            return Err(EvidenceError::classified(
                "intent_capacity",
                "postgres_representation",
            ));
        }
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
                        EXECUTION_INSERT,
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
        let length = i32::try_from(bytes.len())
            .map_err(|error| EvidenceError::source("intent_readback_length", error))?;
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
            FROM reborn_original_pure_case_executions WHERE case_id=$1
                AND octet_length(execution_bytes)=$2 AND checksum=$3",
                &[&case_id, &length, &&digest[..]],
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
        if !brassclaw_pg::wire_capacity::text_record_fits(
            OBSERVATION_INSERT.len(),
            &[16, 32, observation.len(), native.len(), 32],
        ) || originals.iter().any(|(stage, r)| {
            !brassclaw_pg::wire_capacity::text_record_fits(
                RECEIPT_INSERT.len(),
                &[16, stage.len(), 8, 32, 16, r.bytes().len(), 32],
            )
        }) {
            return Err(EvidenceError::classified(
                "observation_capacity",
                "postgres_representation",
            ));
        }
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
                TransactionClient::from_client(self.pool.get().await.map_err(|error| {
                    EvidenceError::source("observation_write_connection", error)
                })?);
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .map_err(|error| EvidenceError::database("observation_write", error))?;
            for (stage, receipt) in originals {
                let ordinal = i64::try_from(receipt.ordinal())
                    .map_err(|error| EvidenceError::source("receipt_ordinal", error))?;
                tx.execute(
                    RECEIPT_INSERT,
                    &[
                        &case_id,
                        stage,
                        &ordinal,
                        &&execution[..],
                        &receipt.request_id(),
                        &receipt.bytes(),
                        &&receipt.checksum()[..],
                    ],
                )
                .await
                .map_err(|error| EvidenceError::database("observation_write", error))?;
            }
            tx.execute(
                OBSERVATION_INSERT,
                &[
                    &case_id,
                    &&execution[..],
                    &observation,
                    &native,
                    &&digest[..],
                ],
            )
            .await
            .map_err(|error| EvidenceError::database("observation_write", error))?;
            tx.commit()
                .await
                .map_err(|error| EvidenceError::database("observation_write_commit", error))?;
            client.recycle_after_commit();
            Ok(())
        })
        .await
        .map_err(|_| EvidenceError::classified("observation_write", "deadline"))
        .and_then(|result| result);
        let read = tokio::time::timeout(timeout, async {
            let observation_length=i32::try_from(observation.len())
                .map_err(|error|EvidenceError::source("observation_readback_length",error))?;
            let native_length=i32::try_from(native.len())
                .map_err(|error|EvidenceError::source("observation_readback_length",error))?;
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
                FROM reborn_original_pure_case_observations WHERE case_id=$1
                    AND octet_length(observation_bytes)=$2 AND octet_length(native_receipts_bytes)=$3 AND checksum=$4",
                    &[&case_id, &observation_length, &native_length, &&digest[..]],
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
            let count:i64=tx.query_one("SELECT count(*) FROM reborn_original_pure_case_native_receipts WHERE case_id=$1",&[&case_id])
                .await.map_err(|error|EvidenceError::database("observation_readback",error))?.get(0);
            if usize::try_from(count).ok()!=Some(originals.len()) {
                return Err(EvidenceError::classified("observation_readback","exact_mismatch"));
            }
            // One authenticated original at a time: do not materialize every
            // full IPC boundary again in one Vec<Row>. The row's actual known
            // size/hash is filtered before its text is transferred/decoded.
            for (stage,receipt) in originals {
                let length=i32::try_from(receipt.bytes().len()).map_err(|error|EvidenceError::source("receipt_length",error))?;
                let row=tx.query_opt("SELECT ordinal,execution_checksum,request_id,receipt_bytes,checksum
                    FROM reborn_original_pure_case_native_receipts WHERE case_id=$1 AND stage=$2
                    AND octet_length(receipt_bytes)=$3 AND checksum=$4",&[&case_id,stage,&length,&&receipt.checksum()[..]])
                    .await.map_err(|error|EvidenceError::database("observation_readback",error))?
                    .ok_or_else(||EvidenceError::classified("observation_readback","exact_mismatch"))?;
                if row.get::<_,i64>(0)!=i64::try_from(receipt.ordinal()).map_err(|error|EvidenceError::source("receipt_ordinal",error))?
                    || row.get::<_,Vec<u8>>(1).as_slice()!=execution
                    || row.get::<_,Uuid>(2)!=receipt.request_id()
                    || row.get::<_,&str>(3)!=receipt.bytes()
                    || row.get::<_,Vec<u8>>(4).as_slice()!=receipt.checksum() {
                    return Err(EvidenceError::classified("observation_readback","exact_mismatch"));
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
        if !brassclaw_pg::wire_capacity::text_record_fits(
            FAILURE_INSERT.len(),
            &[16, 16, phase.len(), bytes.len(), 32],
        ) {
            return Err(EvidenceError::classified(
                "failure_capacity",
                "postgres_representation",
            ));
        }
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
                        EvidenceError::source("failure_write_connection", error)
                    })?);
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .start()
                    .await
                    .map_err(|error| EvidenceError::database("failure_write", error))?;
                tx.execute(
                    FAILURE_INSERT,
                    &[&case_id, &invocation_id, &phase, &bytes, &&digest[..]],
                )
                .await
                .map_err(|error| EvidenceError::database("failure_write", error))?;
                tx.commit()
                    .await
                    .map_err(|error| EvidenceError::database("failure_write_commit", error))?;
                client.recycle_after_commit();
                Ok::<(), EvidenceError>(())
            })
            .await
            .map_err(|_| EvidenceError::classified("failure_write", "deadline"))
            .and_then(|result| result);
        let read=tokio::time::timeout(timeout, async {
            let length=i32::try_from(bytes.len()).map_err(|error|EvidenceError::source("failure_readback_length",error))?;
            let mut client=TransactionClient::from_client(self.pool.get().await
                .map_err(|error|EvidenceError::source("failure_readback_connection",error))?);
            let tx=client.build_transaction().isolation_level(IsolationLevel::RepeatableRead).read_only(true).start().await
                .map_err(|error|EvidenceError::database("failure_readback",error))?;
            let row=tx.query_opt("SELECT invocation_id,failure_bytes,checksum FROM reborn_original_pure_case_failures
                WHERE case_id=$1 AND phase=$2 AND octet_length(failure_bytes)=$3 AND checksum=$4", &[&case_id,&phase,&length,&&digest[..]]).await
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

#[cfg(test)]
mod encoding_tests {
    use super::*;
    #[test]
    fn original_policy_counts_escaping_before_allocating_complete_output() {
        let value = serde_json::json!({"quoted":"\\\"Ü\n".repeat(1024)});
        let exact = brassclaw_common::json::count_json_bytes(&value, None).unwrap();
        let encoded = encode_original(&value, exact).unwrap();
        assert_eq!(encoded.len() as u64, exact);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
            value
        );
        assert!(matches!(
            encode_original(&value, exact - 1),
            Err(CaseEncodingError::PolicyCapacity)
        ));
        assert!(matches!(
            encode_original_for(
                &value,
                exact,
                "SELECT $1,$2",
                &[brassclaw_pg::wire_capacity::maximum_text_content_bytes()]
            ),
            Err(CaseEncodingError::Representation)
        ));
    }
    #[test]
    fn borrowed_seed_envelope_preserves_existing_exact_bytes() {
        let id = Uuid::new_v4();
        let components = [serde_json::json!({"id":id,"version":7})];
        let report = serde_json::json!({"text":"quotes \"\\ Ü","data":[null,true,7]});
        let original = crate::bootstrap_evidence::system_record(
            id,
            "behavior",
            false,
            "checksum",
            &components,
            report.clone(),
        );
        let borrowed = crate::bootstrap_evidence::system_record_view(
            id,
            "behavior",
            false,
            "checksum",
            &components,
            &report,
        );
        assert_eq!(
            encode_original(&borrowed, original.len() as u64).unwrap(),
            original
        );
    }
    #[test]
    fn serialization_growth_between_count_and_write_cannot_grow_reserved_output() {
        struct Changing(std::cell::Cell<bool>);
        impl serde::Serialize for Changing {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let next = self.0.replace(true);
                serializer.serialize_str(if next { "longer second encoding" } else { "x" })
            }
        }
        assert!(matches!(
            encode_original(&Changing(std::cell::Cell::new(false)), 1024),
            Err(CaseEncodingError::Encoding(_))
        ));
    }
}
