//! Post-Factory delivery evidence. Historical reads confer neither native
//! ownership/release nor Q1, execution permission or a reconstructed live proof.
use super::*;
use brassclaw_turns::private_work::{PrivateExecutionContext, PrivateWorkStatus};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Delivery {
    format: String,
    scope: String,
    work_id: Uuid,
    run_id: Uuid,
    terminal_checksum: [u8; 32],
}

pub(crate) struct DeliveredPrivateCompletion {
    original: PrivateCompletionOriginal,
    native: terminal::HistoricalNative,
    checksum: [u8; 32],
    outcome: Value,
    report: Value,
}
impl DeliveredPrivateCompletion {
    pub(crate) fn original(&self) -> &PrivateCompletionOriginal {
        &self.original
    }
    pub(crate) fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub(crate) fn task(&self) -> brassclaw_monty_host::process::TaskHandle {
        self.native.task
    }
    pub(crate) fn outcome(&self) -> &Value {
        &self.outcome
    }
    pub(crate) fn report(&self) -> &Value {
        &self.report
    }
}

fn invalid() -> AgentLoopDriverError {
    failed("private_delivery_original_invalid")
}
fn database(_: tokio_postgres::Error) -> AgentLoopDriverError {
    failed("private_delivery_database_failed")
}
fn digest(bytes: &str) -> [u8; 32] {
    Sha256::digest(bytes.as_bytes()).into()
}

impl PgMontyAdmission {
    /// Called only by the driver after the actual factory acknowledgement.
    /// The private live-proof constructor is inaccessible to this store.
    pub(crate) async fn record_private_delivery(
        pool: &PgPool,
        completion: &crate::global_monty_driver::SettledInternalCompletion,
    ) -> Result<(), AgentLoopDriverError> {
        let mut client = crate::bootstrap_evidence::io::EvidenceClient::acquire(pool)
            .await
            .map_err(|_| failed("private_delivery_database_failed"))?;
        let tx = client.transaction().await.map_err(database)?;
        let Some(original) = Self::verify_private_live_completion(&tx, completion).await? else {
            return Err(invalid());
        };
        let request = original.request();
        let row = tx.query_one("SELECT private_terminal_checksum FROM brassclaw_turns
            WHERE work_id=$1 AND execution_run_id=$2 AND status='settled' AND private_cancel_revision=0",
            &[&request.work_id(), &request.run_id().as_uuid()]).await.map_err(database)?;
        let terminal_checksum: [u8; 32] =
            row.get::<_, Vec<u8>>(0).try_into().map_err(|_| invalid())?;
        let record = Delivery {
            format: "monty-private-delivery/1".into(),
            scope: "native-post-factory-acknowledgement".into(),
            work_id: request.work_id(),
            run_id: request.run_id().as_uuid(),
            terminal_checksum,
        };
        let bytes = serde_json::to_string(&record).map_err(|_| invalid())?;
        let checksum = digest(&bytes);
        tx.execute("INSERT INTO brassclaw_monty_private_deliveries(work_id,run_id,terminal_checksum,delivery_bytes,checksum)
            VALUES($1,$2,$3,$4,$5) ON CONFLICT(work_id) DO NOTHING",
            &[&record.work_id,&record.run_id,&&terminal_checksum[..],&bytes,&&checksum[..]])
            .await.map_err(database)?;
        let row = tx
            .query_one(
                "SELECT delivery_bytes,checksum FROM brassclaw_monty_private_deliveries
            WHERE work_id=$1 AND run_id=$2",
                &[&record.work_id, &record.run_id],
            )
            .await
            .map_err(database)?;
        if row.get::<_, &str>(0) != bytes || row.get::<_, Vec<u8>>(1).as_slice() != checksum {
            return Err(invalid());
        }
        tx.commit().await.map_err(database)?;
        client.recycle_after_commit();
        Ok(())
    }

    /// Exact expected context is mandatory even for historical, read-only use.
    /// None is absence of an acknowledgement, never proof of no native effect.
    pub(crate) async fn read_private_delivery(
        tx: &Transaction<'_>,
        work_id: Uuid,
        context: &PrivateExecutionContext,
    ) -> Result<Option<DeliveredPrivateCompletion>, AgentLoopDriverError> {
        let Some(state) =
            brassclaw_turns::PgTurnStateStore::read_private_work_state_in_transaction(
                tx,
                context.tenant_id.as_str(),
                work_id,
            )
            .await
            .map_err(|_| invalid())?
        else {
            return Ok(None);
        };
        if state.request.context() != context || state.request.work_id() != work_id {
            return Err(invalid());
        }
        let Some(row) = tx.query_opt("SELECT d.delivery_bytes,d.checksum,d.terminal_checksum,
            p.definition_bytes,p.checksum,t.terminal_bytes,t.execution_report_bytes,
            a.runner_id,a.claim_checksum,a.admission_key,a.private_claim_generation,a.private_request_checksum,
            a.outcome,a.private_execution_context,a.native_actor,w.private_definition_checksum,w.private_claim_checksum,
            w.private_claim_generation,w.private_runner_id,w.private_admission_run
            FROM brassclaw_monty_private_deliveries d
            JOIN brassclaw_monty_private_preparations p USING(work_id)
            JOIN brassclaw_monty_private_terminals t ON t.work_id=d.work_id AND t.run_id=d.run_id AND t.checksum=d.terminal_checksum
            JOIN brassclaw_monty_task_admissions a ON a.run_id=d.run_id AND a.private_work_id=d.work_id
            JOIN brassclaw_turns w ON w.work_id=d.work_id AND w.execution_run_id=d.run_id
            WHERE d.work_id=$1 AND d.run_id=$2 AND a.origin_kind='private_work' AND a.phase='settled'",
            &[&work_id,&state.request.run_id().as_uuid()]).await.map_err(database)? else {
            if tx.query_one("SELECT EXISTS(SELECT 1 FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
                &[&work_id]).await.map_err(database)?.get::<_, bool>(0) { return Err(invalid()); }
            return Ok(None);
        };
        if state.status != PrivateWorkStatus::Settled || state.cancellation_revision != 0 {
            return Err(invalid());
        }
        let bytes: &str = row.get(0);
        let checksum: [u8; 32] = row.get::<_, Vec<u8>>(1).try_into().map_err(|_| invalid())?;
        if digest(bytes) != checksum {
            return Err(invalid());
        }
        let delivery: Delivery = serde_json::from_str(bytes).map_err(|_| invalid())?;
        if delivery.format != "monty-private-delivery/1"
            || delivery.scope != "native-post-factory-acknowledgement"
            || delivery.work_id != work_id
            || delivery.run_id != state.request.run_id().as_uuid()
            || Some(delivery.terminal_checksum) != state.terminal_checksum
            || row.get::<_, Vec<u8>>(2).as_slice() != delivery.terminal_checksum
        {
            return Err(invalid());
        }
        let definition_checksum: [u8; 32] =
            row.get::<_, Vec<u8>>(4).try_into().map_err(|_| invalid())?;
        let definition = crate::private_work_definition::PrivateWorkDefinition::restore(
            &state.request,
            state.request_checksum,
            row.get(3),
            definition_checksum,
        )
        .map_err(|_| invalid())?;
        let report_bytes: &str = row.get::<_, Option<&str>>(6).ok_or_else(invalid)?;
        let native =
            terminal::Terminal::read_success(row.get(5), delivery.terminal_checksum, report_bytes)?;
        let report: Value = serde_json::from_str(report_bytes).map_err(|_| invalid())?;
        let outcome: Value = row.get::<_, Option<Value>>(12).ok_or_else(invalid)?;
        let address = terminal::Address {
            work_id,
            run_id: state.request.run_id().as_uuid(),
            claim_generation: 1,
            claim_checksum: row.get::<_, Vec<u8>>(8).try_into().map_err(|_| invalid())?,
            request_checksum: state.request_checksum,
            definition_checksum,
            admission_ref: Sha256::digest(row.get::<_, Vec<u8>>(9)).into(),
            cancellation_revision: 0,
        };
        if native.address != address
            || row.get::<_, i64>(10) != 1
            || row.get::<_, i64>(17) != 1
            || row.get::<_, Vec<u8>>(11).as_slice() != state.request_checksum
            || row.get::<_, Value>(13) != serde_json::to_value(context).map_err(|_| invalid())?
            || row.get::<_, Value>(14)
                != serde_json::to_value(&context.actor).map_err(|_| invalid())?
            || row.get::<_, Vec<u8>>(15).as_slice() != definition_checksum
            || row.get::<_, Vec<u8>>(16).as_slice() != address.claim_checksum
            || row.get::<_, Uuid>(18) != row.get::<_, Uuid>(7)
            || row.get::<_, Uuid>(19) != delivery.run_id
            || native.root["source_checksum"] != hex::encode(definition.root().source_checksum())
            || outcome["status"] != "completed"
            || outcome.get("execution") != Some(&report)
            || outcome["internal_receipt"].as_str().map(digest) != Some(native.receipt_checksum)
            || report["format"] != "monty-internal-task-execution/1"
            || report["root"] != native.root
            || report["accounting"] != native.accounting
            || report["root_completed"] != true
            || report["all_selected_recipes_complete"] != true
            || report["internal_receipt"] != outcome["internal_receipt"]
            || !report["reply"].is_null()
            || report["withheld_root_answers"] != 0
            || report["withheld_host_answers"] != false
            || report["semantic_approval"] != false
            || report["catalogue_activation"] != false
            || Self::read_private_invocation_refs(tx, state.request.run_id()).await?
                != native.reconciliation_refs
        {
            return Err(invalid());
        }
        Self::validate_private_result_reference(&outcome, state.request.run_id())?;
        Ok(Some(DeliveredPrivateCompletion {
            original: PrivateCompletionOriginal {
                request: state.request,
                definition,
            },
            native,
            checksum,
            outcome,
            report,
        }))
    }
    fn validate_private_result_reference(
        outcome: &Value,
        run: brassclaw_turns::TurnRunId,
    ) -> Result<(), AgentLoopDriverError> {
        let reference = crate::monty_result_reference::result_reference(
            run,
            outcome["internal_receipt"].as_str().ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)?;
        if outcome["result_ref"] != reference.as_str() {
            return Err(invalid());
        }
        Ok(())
    }
}
