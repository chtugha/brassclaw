//! Case-owner acknowledgement of the exact normal H01 delivery. Metadata only;
//! this module cannot dispatch, resume, approve or repeat a candidate invocation.
use brassclaw_pg::PgPool;
use brassclaw_turns::{
    private_work::{PrivateExecutionContext, PrivateOriginalRef},
    run_profile::AgentLoopDriverError,
};
use serde::{Deserialize, Serialize};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

use crate::{
    pg_transaction_client::TransactionClient, private_work_definition::checksum,
    pure_case_original::CheckedOriginalPureCase,
    validator_inventory_review::QualifiedPureSourceReview,
};

fn invalid() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "original_pure_case_settlement_invalid".into(),
    }
}
fn unavailable() -> AgentLoopDriverError {
    AgentLoopDriverError::Unavailable {
        reason: "original Pure case settlement metadata unavailable".into(),
    }
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Settlement {
    format: String,
    case_id: Uuid,
    case_checksum: [u8; 32],
    execution_checksum: [u8; 32],
    observation_checksum: [u8; 32],
    delivery_checksum: [u8; 32],
}
/// Historical case result issued only after rechecking the original Source,
/// native invocation and genuine post-Factory Harness delivery. It is not live
/// execution authority, oracle adequacy, Q1 or a publication approval.
pub(crate) struct SettledOriginalPureCase {
    original: CheckedOriginalPureCase,
    observation_bytes: String,
    checksum: [u8; 32],
}
impl SettledOriginalPureCase {
    pub(crate) fn original(&self) -> &CheckedOriginalPureCase {
        &self.original
    }
    pub(crate) fn observation_bytes(&self) -> &str {
        &self.observation_bytes
    }
    pub(crate) fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
}
struct Prepared {
    original: CheckedOriginalPureCase,
    observation_bytes: String,
    record: Settlement,
    bytes: String,
    checksum: [u8; 32],
}
async fn prepare(
    tx: &Transaction<'_>,
    id: Uuid,
    context: &PrivateExecutionContext,
    source: &QualifiedPureSourceReview,
) -> Result<Option<Prepared>, AgentLoopDriverError> {
    let case = CheckedOriginalPureCase::read(tx, source, id, context)
        .await
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let Some(delivery) =
        crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(tx, id, context).await?
    else {
        return Ok(None);
    };
    if !matches!(delivery.original().request().original(),PrivateOriginalRef::OriginalPureCaseV1 {case_id,case_checksum}
        if *case_id==id && *case_checksum==case.checksum())
    {
        return Err(invalid());
    }
    let bounds = delivery
        .original()
        .definition()
        .preparation()
        .limits()
        .bounds;
    let (expected_source, expected_inputs) = case
        .expected_start(brassclaw_skills::value_contract::ContractLimits {
            max_depth: bounds.max_value_depth,
            max_nodes: bounds.max_value_nodes,
            max_bytes: bounds.max_value_bytes,
        })
        .map_err(|_| invalid())?;
    // Compare the normal encoder's complete JSON value in PostgreSQL; do not
    // parse/re-encode arbitrary candidate data with a second host depth limit.
    let row=tx.query_opt("SELECT e.checksum,o.checksum,o.observation_bytes,o.native_receipts_bytes
        FROM reborn_original_pure_case_executions e
        JOIN reborn_original_pure_case_observations o USING(case_id)
        JOIN reborn_original_pure_case_native_receipts s ON s.case_id=e.case_id AND s.stage='start'
        JOIN brassclaw_monty_tool_invocations i ON i.invocation_id=e.invocation_id AND i.run_id=e.run_id
        JOIN brassclaw_monty_task_admissions a ON a.run_id=e.run_id
        WHERE e.case_id=$1 AND i.phase='answered' AND i.attempt_count=1
            AND i.selection_bytes::jsonb->>'capability_id'='builtin.pure_review.invoke_original'
            AND i.arguments_bytes::jsonb='{}'::jsonb
            AND i.answer_bytes::jsonb=jsonb_build_object('kind','return','value',o.observation_bytes)
            AND (a.outcome->>'internal_receipt')::jsonb=jsonb_build_object(
                'format','original-pure-case-receipt/1','observation',o.observation_bytes::jsonb)
            AND e.execution_bytes::jsonb->'task'=$2::jsonb
            AND s.receipt_bytes::jsonb->'command'->'command'->'selected'->>'source'=$3
            AND s.receipt_bytes::jsonb->'command'->'command'->'selected'->'aliases'='[]'::jsonb
            AND s.receipt_bytes::jsonb->'command'->'command'->'inputs'=$4::jsonb
            AND o.observation_bytes::jsonb->'invocation_checksum'=
                (SELECT jsonb_agg(get_byte(sha256(convert_to($3,'UTF8')),k) ORDER BY k) FROM generate_series(0,31)k)
            AND original_pure_case_observation_valid(e.case_id,e.checksum,o.observation_bytes,o.native_receipts_bytes)",
        &[&id,&serde_json::to_value(delivery.task()).map_err(|_|invalid())?,&expected_source,&expected_inputs]).await.map_err(|_|unavailable())?.ok_or_else(invalid)?;
    let execution_checksum = row.get::<_, Vec<u8>>(0).try_into().map_err(|_| invalid())?;
    let observation_checksum: [u8; 32] =
        row.get::<_, Vec<u8>>(1).try_into().map_err(|_| invalid())?;
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(row.get::<_, &str>(2).as_bytes());
    digest.update(row.get::<_, &str>(3).as_bytes());
    if <[u8; 32]>::from(digest.finalize()) != observation_checksum {
        return Err(invalid());
    }
    let record = Settlement {
        format: "settled-original-pure-case/1".into(),
        case_id: id,
        case_checksum: case.checksum(),
        execution_checksum,
        observation_checksum,
        delivery_checksum: delivery.checksum(),
    };
    let bytes = serde_json::to_string(&record).map_err(|_| invalid())?;
    Ok(Some(Prepared {
        original: case,
        observation_bytes: row.get::<_, String>(2),
        checksum: checksum(&bytes),
        record,
        bytes,
    }))
}
impl Prepared {
    fn into_settled(self) -> SettledOriginalPureCase {
        SettledOriginalPureCase {
            original: self.original,
            observation_bytes: self.observation_bytes,
            checksum: self.checksum,
        }
    }
}
async fn require_record(
    tx: &Transaction<'_>,
    prepared: &Prepared,
) -> Result<bool, AgentLoopDriverError> {
    let Some(row) = tx
        .query_opt(
            "SELECT observation_checksum,delivery_checksum,settlement_bytes,checksum
        FROM reborn_original_pure_case_settlements WHERE case_id=$1",
            &[&prepared.record.case_id],
        )
        .await
        .map_err(|_| unavailable())?
    else {
        return Ok(false);
    };
    let bytes: &str = row.get(2);
    let decoded: Settlement = serde_json::from_str(bytes).map_err(|_| invalid())?;
    if decoded != prepared.record
        || bytes != prepared.bytes
        || checksum(bytes) != prepared.checksum
        || row.get::<_, Vec<u8>>(0).as_slice() != prepared.record.observation_checksum
        || row.get::<_, Vec<u8>>(1).as_slice() != prepared.record.delivery_checksum
        || row.get::<_, Vec<u8>>(3).as_slice() != prepared.checksum
    {
        return Err(invalid());
    }
    Ok(true)
}
/// Historical read rechecks the actual I01 source original, normal delivery and
/// exact native/encoder relationship. It creates no live proof or new approval.
pub(crate) async fn read(
    tx: &Transaction<'_>,
    id: Uuid,
    context: &PrivateExecutionContext,
    source: &QualifiedPureSourceReview,
) -> Result<Option<SettledOriginalPureCase>, AgentLoopDriverError> {
    match prepare(tx, id, context, source).await? {
        Some(prepared) => {
            if require_record(tx, &prepared).await? {
                Ok(Some(prepared.into_settled()))
            } else {
                Ok(None)
            }
        }
        None => {
            if tx.query_one("SELECT EXISTS(SELECT 1 FROM reborn_original_pure_case_settlements WHERE case_id=$1)",&[&id])
                .await.map_err(|_|unavailable())?.get::<_,bool>(0) {return Err(invalid());}
            Ok(None)
        }
    }
}
/// One original acknowledgement plus one fresh readback on uncertainty. Each
/// transaction gets the caller's acknowledged store timeout; no worker replay.
pub(crate) async fn settle(
    pool: &PgPool,
    id: Uuid,
    context: &PrivateExecutionContext,
    source: &QualifiedPureSourceReview,
    timeout: std::time::Duration,
) -> Result<Option<SettledOriginalPureCase>, AgentLoopDriverError> {
    let write=tokio::time::timeout(timeout,async {
        let mut client=TransactionClient::from_client(pool.get().await.map_err(|_|unavailable())?);
        let tx=client.build_transaction().isolation_level(IsolationLevel::RepeatableRead).start().await.map_err(|_|unavailable())?;
        let result=if let Some(prepared)=prepare(&tx,id,context,source).await? {
            tx.execute("INSERT INTO reborn_original_pure_case_settlements(case_id,observation_checksum,delivery_checksum,settlement_bytes,checksum)
                VALUES($1,$2,$3,$4,$5) ON CONFLICT(case_id) DO NOTHING",&[&id,&&prepared.record.observation_checksum[..],
                &&prepared.record.delivery_checksum[..],&prepared.bytes,&&prepared.checksum[..]]).await.map_err(|_|unavailable())?;
            if !require_record(&tx,&prepared).await? {return Err(invalid());} Some(prepared)
        } else {None};
        tx.commit().await.map_err(|_|unavailable())?; client.recycle_after_commit(); Ok(result.map(Prepared::into_settled))
    }).await.unwrap_or_else(|_|Err(unavailable()));
    match write {
        Ok(result) => Ok(result),
        Err(primary) => {
            let readback = tokio::time::timeout(timeout, async {
                let mut client =
                    TransactionClient::from_client(pool.get().await.map_err(|_| unavailable())?);
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(|_| unavailable())?;
                let present = read(&tx, id, context, source).await?;
                tx.commit().await.map_err(|_| unavailable())?;
                client.recycle_after_commit();
                Ok(present)
            })
            .await
            .unwrap_or_else(|_| Err(unavailable()));
            match readback {
                Ok(Some(original)) => Ok(Some(original)),
                _ => Err(primary),
            }
        }
    }
}
