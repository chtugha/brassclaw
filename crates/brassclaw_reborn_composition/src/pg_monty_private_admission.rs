//! Genuine private origin in the existing admission journal, never a chat alias.
use super::*;
use brassclaw_turns::private_work::ClaimedPrivateWork;
use uuid::Uuid;

#[cfg(test)]
#[path = "pg_private_settlement_observation.rs"]
mod settlement_observation;
#[cfg(test)]
pub(crate) use settlement_observation::{SettlementPhase, SettlementScope};

/// Access contract chosen by the existing owner, never inferred from a failed
/// SQL statement. Historical readers share one coherent snapshot; terminal and
/// delivery writers retain their invocation-row custody until COMMIT.
#[derive(Clone, Copy)]
enum InvocationRefsRead {
    Snapshot,
    RetainShareLock,
}

/// Passive facts about one original intent exchange. Each bit is published only
/// at the actual boundary; inspection neither polls nor holds the exchange.
#[cfg(test)]
#[derive(Default)]
pub(super) struct IntentObservation {
    reached: std::sync::atomic::AtomicU16,
    backend_pid: std::sync::atomic::AtomicI32,
    settlement: settlement_observation::SettlementObservation,
}
#[cfg(test)]
#[derive(Clone, Copy)]
enum IntentBoundary {
    AcquireStarted,
    AcquireAcknowledged,
    BeginStarted,
    BeginAcknowledged,
    ClaimStarted,
    ClaimAcknowledged,
    UpdateStarted,
    UpdateAcknowledged,
    CommitStarted,
    CommitAcknowledged,
    ClientScopeEnded,
}
#[cfg(test)]
struct IntentClientScope<'a> {
    observation: &'a IntentObservation,
    acquired: bool,
}
#[cfg(test)]
impl Drop for IntentClientScope<'_> {
    fn drop(&mut self) {
        if self.acquired {
            // Declared before the client, hence dropped after it. This observes
            // Rust disposal only, never PostgreSQL rollback/disconnection ACK.
            self.observation.note(IntentBoundary::ClientScopeEnded);
        }
    }
}
#[cfg(test)]
impl IntentObservation {
    fn note(&self, boundary: IntentBoundary) {
        self.reached
            .fetch_or(1 << boundary as u16, std::sync::atomic::Ordering::Release);
    }
    fn claim_backend(&self, pid: i32) {
        let reached = self.reached.load(std::sync::atomic::Ordering::Acquire);
        if reached & (1 << IntentBoundary::ClaimStarted as u16) != 0
            && reached & (1 << IntentBoundary::ClaimAcknowledged as u16) == 0
        {
            self.backend_pid
                .store(pid, std::sync::atomic::Ordering::Release);
        }
    }
    fn snapshot(&self) -> Value {
        let reached = self.reached.load(std::sync::atomic::Ordering::Acquire);
        let names = [
            "acquire_started",
            "acquire_acknowledged",
            "begin_started",
            "begin_acknowledged",
            "claim_started",
            "claim_acknowledged",
            "update_started",
            "update_acknowledged",
            "commit_started",
            "commit_acknowledged",
            "client_scope_ended",
        ];
        let boundaries: Vec<_> = names
            .into_iter()
            .enumerate()
            .filter_map(|(bit, name)| (reached & (1 << bit) != 0).then_some(name))
            .collect();
        let pid = self.backend_pid.load(std::sync::atomic::Ordering::Acquire);
        serde_json::json!({
            "last_io_boundary": boundaries.iter().rev().copied()
                .find(|boundary| *boundary != "client_scope_ended").unwrap_or("not_started"),
            "boundaries": boundaries,
            "client_scope_ended": reached & (1 << IntentBoundary::ClientScopeEnded as u16) != 0,
            "claim_query_backend_pid": (pid > 0).then_some(pid),
            "settlement": self.settlement.snapshot(),
        })
    }
}

impl PgMontyAdmission {
    pub(super) async fn inspect_private_claim(
        &self,
        transaction: &Transaction<'_>,
        claimed: &ClaimedPrivateWork,
        retain_lock: bool,
    ) -> Result<(), AgentLoopDriverError> {
        let mut query = String::from(
            "SELECT private_request_checksum,private_definition_checksum,
            private_claim_checksum,private_runner_id,private_claim_generation,
            private_cancel_revision,private_lease_deadline>clock_timestamp() AS live,
            execution_run_id,private_admission_run",
        );
        #[cfg(test)]
        query.push_str(",pg_backend_pid() AS observer_backend_pid");
        query.push_str(
            " FROM brassclaw_turns
            WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2 AND status='claimed'",
        );
        if retain_lock {
            query.push_str(" FOR UPDATE");
        }
        let row = transaction
            .query_opt(
                &query,
                &[
                    &claimed.request().context().tenant_id.as_str(),
                    &claimed.request().work_id(),
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .ok_or_else(|| failed("monty_admission_fenced"))?;
        #[cfg(test)]
        self.intent_observation
            .claim_backend(row.get("observer_backend_pid"));
        if row.get::<_, Vec<u8>>(0).as_slice() != claimed.request_checksum()
            || row.get::<_, Vec<u8>>(1).as_slice() != claimed.definition_checksum()
            || row.get::<_, Vec<u8>>(2).as_slice() != self.checksum
            || row.get::<_, uuid::Uuid>(3) != self.attempt.runner_id.as_uuid()
            || row.get::<_, i64>(4)
                != i64::try_from(claimed.generation())
                    .map_err(|_| failed("monty_admission_fenced"))?
            || row.get::<_, i64>(5) != 0
            || !row.get::<_, bool>(6)
            || row.get::<_, uuid::Uuid>(7) != self.attempt.run_id.as_uuid()
            || row.get::<_, uuid::Uuid>(8) != self.attempt.run_id.as_uuid()
            || claimed.attempt() != self.attempt
        {
            return Err(failed("monty_admission_fenced"));
        }
        Ok(())
    }

    /// Persist intent before the first possible submit exchange. An uncertain
    /// acknowledgement stays fenced; missing receipt is never no-effect proof.
    pub(crate) fn private_intent_retirement(
        &self,
        timeout: std::time::Duration,
    ) -> Arc<brassclaw_pg::transaction_client::retirement::TransactionRetirement> {
        brassclaw_pg::transaction_client::retirement::TransactionRetirement::new(
            self.pool.as_ref().clone(),
            timeout,
        )
    }

    pub(crate) async fn mark_private_submission_intent(
        &self,
        storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
        retirement: &Arc<brassclaw_pg::transaction_client::retirement::TransactionRetirement>,
    ) -> Result<(), AgentLoopDriverError> {
        crate::bootstrap_evidence::io::bounded_for(
            storage,
            "private_submission_intent",
            self.attempt.run_id.as_uuid(),
            async { Ok(self.write_private_submission_intent(retirement).await) },
        )
        .await
        .map_err(|error| AgentLoopDriverError::Unavailable {
            reason: error.to_string(),
        })?
    }

    async fn write_private_submission_intent(
        &self,
        retirement: &Arc<brassclaw_pg::transaction_client::retirement::TransactionRetirement>,
    ) -> Result<(), AgentLoopDriverError> {
        let AdmissionOrigin::Private(_) = &self.origin else {
            return Ok(());
        };
        #[cfg(test)]
        self.intent_observation.note(IntentBoundary::AcquireStarted);
        #[cfg(test)]
        let mut client_scope = IntentClientScope {
            observation: &self.intent_observation,
            acquired: false,
        };
        let mut client = retirement
            .acquire()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        {
            client_scope.acquired = true;
        }
        #[cfg(test)]
        self.intent_observation
            .note(IntentBoundary::AcquireAcknowledged);
        retirement
            .capture_backend(&client)
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        self.intent_observation.note(IntentBoundary::BeginStarted);
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        self.intent_observation
            .note(IntentBoundary::BeginAcknowledged);
        #[cfg(test)]
        self.intent_observation.note(IntentBoundary::ClaimStarted);
        self.verify_claim(&transaction).await?;
        #[cfg(test)]
        self.intent_observation
            .note(IntentBoundary::ClaimAcknowledged);
        #[cfg(test)]
        self.intent_observation.note(IntentBoundary::UpdateStarted);
        let changed=transaction.execute("UPDATE brassclaw_monty_task_admissions
            SET private_submission_intent_at=clock_timestamp()
            WHERE run_id=$1 AND admission_key=$2 AND claim_checksum=$3
            AND origin_kind='private_work' AND phase='reserved' AND private_submission_intent_at IS NULL",
            &[&self.attempt.run_id.as_uuid(),&&self.key[..],&&self.checksum[..]])
            .await.map_err(|_|failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        self.intent_observation
            .note(IntentBoundary::UpdateAcknowledged);
        if changed != 1 {
            return Err(failed("monty_admission_replay_requires_recovery"));
        }
        #[cfg(test)]
        self.intent_observation.note(IntentBoundary::CommitStarted);
        let committed = transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"));
        #[cfg(test)]
        if committed.is_ok() {
            self.intent_observation
                .note(IntentBoundary::CommitAcknowledged);
        }
        committed?;
        client.recycle_after_commit();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn test_private_intent_observation(&self) -> Value {
        self.intent_observation.snapshot()
    }
    #[cfg(test)]
    pub(crate) fn test_settlement_started(&self) -> SettlementScope {
        self.intent_observation.settlement.start()
    }
}

#[async_trait::async_trait]
impl brassclaw_turns::pg_turn_state_store::PgPrivateClaimReservation for PgMontyAdmission {
    fn original_claim(&self) -> Result<&ClaimedPrivateWork, brassclaw_turns::TurnError> {
        match &self.origin {
            AdmissionOrigin::Private(claimed) => Ok(claimed.as_ref()),
            AdmissionOrigin::Chat(_) => Err(brassclaw_turns::TurnError::Unavailable {
                reason: "private reservation origin is invalid".into(),
            }),
        }
    }
    async fn reserve_in_transaction(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<(), brassclaw_turns::TurnError> {
        let invalid = || brassclaw_turns::TurnError::Unavailable {
            reason: "private Monty reservation failed".into(),
        };
        let AdmissionOrigin::Private(claimed) = &self.origin else {
            return Err(invalid());
        };
        self.verify_claim(transaction)
            .await
            .map_err(|_| invalid())?;
        let scope = serde_json::to_value(claimed.request().context()).map_err(|_| invalid())?;
        let actor =
            serde_json::to_value(&claimed.request().context().actor).map_err(|_| invalid())?;
        let generation = i64::try_from(claimed.generation()).map_err(|_| invalid())?;
        let changed=transaction.execute("INSERT INTO brassclaw_monty_task_admissions(
            run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase,
            trusted_internal_turn,catalogue_reference_version,task_model_selection_version,native_actor,
            origin_kind,private_work_id,private_execution_context,private_request_checksum,private_claim_generation)
            VALUES($1,NULL,NULL,NULL,$2,$3,$4,'reserved',false,1,1,$5,
                'private_work',$6,$7,$8,$9)",&[
                &self.attempt.run_id.as_uuid(),&self.attempt.runner_id.as_uuid(),&&self.checksum[..],&&self.key[..],
                &actor,&claimed.request().work_id(),&scope,&&claimed.request_checksum()[..],&generation,
            ]).await.map_err(|_|invalid())?;
        if changed != 1 {
            return Err(invalid());
        }
        Ok(())
    }
}

#[path = "private_work_terminal.rs"]
mod terminal;

#[path = "pg_private_delivery.rs"]
mod delivery;
pub(crate) use delivery::DeliveredPrivateCompletion;

impl PgMontyAdmission {
    async fn private_settlement_address(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<terminal::Address, AgentLoopDriverError> {
        let AdmissionOrigin::Private(claimed) = &self.origin else {
            return Err(failed("private_admission_origin_required"));
        };
        let row=transaction.query_opt("SELECT private_request_checksum,private_definition_checksum,
            private_claim_checksum,private_runner_id,private_claim_generation,private_cancel_revision,execution_run_id
            FROM brassclaw_turns WHERE record_kind='private_work' AND tenant_id=$1 AND work_id=$2
            AND status IN ('claimed','reconciliation_required','settled') FOR UPDATE",
            &[&claimed.request().context().tenant_id.as_str(),&claimed.request().work_id()])
            .await.map_err(|_|failed("monty_admission_database_failed"))?.ok_or_else(||failed("monty_admission_fenced"))?;
        if row.get::<_, Vec<u8>>(0).as_slice() != claimed.request_checksum()
            || row.get::<_, Vec<u8>>(1).as_slice() != claimed.definition_checksum()
            || row.get::<_, Vec<u8>>(2).as_slice() != self.checksum
            || row.get::<_, Uuid>(3) != self.attempt.runner_id.as_uuid()
            || row.get::<_, i64>(4) != 1
            || row.get::<_, Uuid>(6) != self.attempt.run_id.as_uuid()
        {
            return Err(failed("monty_admission_fenced"));
        }
        Ok(terminal::Address {
            work_id: claimed.request().work_id(),
            run_id: self.attempt.run_id.as_uuid(),
            claim_generation: claimed.generation(),
            claim_checksum: self.checksum,
            request_checksum: claimed.request_checksum(),
            definition_checksum: claimed.definition_checksum(),
            admission_ref: Sha256::digest(self.key).into(),
            cancellation_revision: u64::try_from(row.get::<_, i64>(5))
                .map_err(|_| failed("monty_admission_fenced"))?,
        })
    }

    /// The native terminal writer retains SHARE locks until its own COMMIT.
    async fn private_invocation_refs(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<Vec<terminal::InvocationRef>, AgentLoopDriverError> {
        Self::read_private_invocation_refs(
            transaction,
            self.attempt.run_id,
            InvocationRefsRead::RetainShareLock,
        )
        .await
    }
    async fn read_private_invocation_refs(
        transaction: &Transaction<'_>,
        run_id: brassclaw_turns::TurnRunId,
        access: InvocationRefsRead,
    ) -> Result<Vec<terminal::InvocationRef>, AgentLoopDriverError> {
        let mut query = String::from(
            "SELECT invocation_id,selection_checksum,arguments_checksum,answer_checksum,phase
            FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY invocation_id",
        );
        if matches!(access, InvocationRefsRead::RetainShareLock) {
            query.push_str(" FOR SHARE");
        }
        let rows = transaction
            .query(&query, &[&run_id.as_uuid()])
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let mut references = Vec::with_capacity(rows.len());
        let digest = |value: &str| {
            crate::private_work_definition::decode_checksum(value)
                .map_err(|_| failed("private_native_reconciliation_invalid"))
        };
        for row in rows {
            if row.get::<_, &str>(4) != "answered" {
                return Err(failed("monty_native_result_reconciliation_required"));
            }
            references.push(terminal::InvocationRef {
                invocation_id: row.get(0),
                selection_checksum: digest(row.get::<_, &str>(1))?,
                arguments_checksum: digest(row.get::<_, &str>(2))?,
                answer_checksum: digest(
                    row.get::<_, Option<&str>>(3)
                        .ok_or_else(|| failed("private_native_reconciliation_invalid"))?,
                )?,
            });
        }
        Ok(references)
    }

    async fn record_private_terminal(
        &self,
        transaction: &Transaction<'_>,
        terminal: &terminal::Terminal,
    ) -> Result<(), AgentLoopDriverError> {
        let AdmissionOrigin::Private(claimed) = &self.origin else {
            return Err(failed("private_admission_origin_required"));
        };
        transaction
            .execute(
                "INSERT INTO brassclaw_monty_private_terminals
            (work_id,run_id,terminal_bytes,checksum,execution_report_bytes)
            VALUES($1,$2,$3,$4,$5) ON CONFLICT(work_id) DO NOTHING",
                &[
                    &claimed.request().work_id(),
                    &self.attempt.run_id.as_uuid(),
                    &terminal.bytes(),
                    &&terminal.checksum()[..],
                    &terminal.report_bytes(),
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let row = transaction
            .query_one(
                "SELECT terminal_bytes,checksum,execution_report_bytes
            FROM brassclaw_monty_private_terminals WHERE work_id=$1 AND run_id=$2",
                &[&claimed.request().work_id(), &self.attempt.run_id.as_uuid()],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        if row.get::<_, &str>(0) != terminal.bytes()
            || row.get::<_, Vec<u8>>(1).as_slice() != terminal.checksum()
            || row.get::<_, Option<&str>>(2) != terminal.report_bytes()
        {
            return Err(failed("monty_admission_settlement_conflict"));
        }
        let changed = transaction
            .execute(
                "UPDATE brassclaw_turns SET status='settled',version=version+1,
            private_terminal_checksum=$3 WHERE record_kind='private_work' AND work_id=$1
            AND execution_run_id=$2 AND status IN ('claimed','reconciliation_required')",
                &[
                    &claimed.request().work_id(),
                    &self.attempt.run_id.as_uuid(),
                    &&terminal.checksum()[..],
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        if changed == 0 {
            let identical: bool = transaction
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM brassclaw_turns
                WHERE record_kind='private_work' AND work_id=$1 AND execution_run_id=$2
                AND status='settled' AND private_terminal_checksum=$3)",
                    &[
                        &claimed.request().work_id(),
                        &self.attempt.run_id.as_uuid(),
                        &&terminal.checksum()[..],
                    ],
                )
                .await
                .map_err(|_| failed("monty_admission_database_failed"))?
                .get(0);
            if !identical {
                return Err(failed("monty_admission_settlement_conflict"));
            }
        }
        Ok(())
    }

    pub(crate) async fn settle_private_native(
        &self,
        outcome: Value,
        receipt: &brassclaw_monty_host::service::TaskReceipt,
        #[cfg(test)] observation: &SettlementScope,
    ) -> Result<(), AgentLoopDriverError> {
        use brassclaw_monty_host::service::TaskOutcome;
        match &receipt.outcome {
            TaskOutcome::InternalCompleted { receipt_ref }
                if outcome["status"] == "completed"
                    && outcome["internal_receipt"].as_str() == Some(receipt_ref.as_str()) => {}
            TaskOutcome::Failed { reason_kind }
                if outcome["status"] == "failed"
                    && outcome["reason_kind"].as_str() == Some(reason_kind.as_str()) => {}
            _ => return Err(failed("private_terminal_original_invalid")),
        }
        #[cfg(test)]
        let mut client_scope = observation.client_scope();
        #[cfg(test)]
        observation.note(SettlementPhase::AcquireStarted);
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        client_scope.acquired(&client);
        #[cfg(test)]
        observation.note(SettlementPhase::BeginStarted);
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        observation.note(SettlementPhase::BeginAcknowledged);
        #[cfg(test)]
        observation.note(SettlementPhase::AddressStarted);
        let address = self.private_settlement_address(&transaction).await?;
        #[cfg(test)]
        observation.note(SettlementPhase::AddressAcknowledged);
        #[cfg(test)]
        observation.note(SettlementPhase::RefsStarted);
        let refs = self.private_invocation_refs(&transaction).await?;
        #[cfg(test)]
        observation.note(SettlementPhase::RefsAcknowledged);
        #[cfg(test)]
        observation.note(SettlementPhase::EncodeStarted);
        let terminal = terminal::Terminal::native(
            address,
            receipt,
            outcome
                .get("execution")
                .ok_or_else(|| failed("private_terminal_original_invalid"))?,
            refs,
        )?;
        #[cfg(test)]
        observation.note(SettlementPhase::EncodeCompleted);
        #[cfg(test)]
        observation.note(SettlementPhase::CasStarted);
        self.settle_in_transaction(&transaction, &outcome).await?;
        #[cfg(test)]
        observation.note(SettlementPhase::CasAcknowledged);
        #[cfg(test)]
        observation.note(SettlementPhase::TerminalStarted);
        self.record_private_terminal(&transaction, &terminal)
            .await?;
        #[cfg(test)]
        observation.note(SettlementPhase::TerminalAcknowledged);
        #[cfg(test)]
        observation.note(SettlementPhase::CommitStarted);
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        #[cfg(test)]
        observation.note(SettlementPhase::CommitAcknowledged);
        Ok(())
    }

    pub(crate) async fn settle_private_unsubmitted(
        &self,
        proof: &brassclaw_monty_host::service::UnsubmittedTask,
    ) -> Result<(), AgentLoopDriverError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let address = self.private_settlement_address(&transaction).await?;
        let terminal = terminal::Terminal::unsubmitted(address, proof)?;
        self.settle_unsubmitted_in_transaction(&transaction, proof.reason())
            .await?;
        self.record_private_terminal(&transaction, &terminal)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))
    }
}

/// Verified metadata of a genuinely live-settled private original. This is not
/// an approval or a proof reconstructed from terminal bytes.
pub(crate) struct PrivateCompletionOriginal {
    request: brassclaw_turns::private_work::PrivateWorkRequest,
    definition: crate::private_work_definition::PrivateWorkDefinition,
}
impl PrivateCompletionOriginal {
    pub(crate) fn request(&self) -> &brassclaw_turns::private_work::PrivateWorkRequest {
        &self.request
    }
    pub(crate) fn definition(&self) -> &crate::private_work_definition::PrivateWorkDefinition {
        &self.definition
    }
}

impl PgMontyAdmission {
    /// Consume the actual driver's opaque native proof in the caller's one
    /// coherent transaction. None means the retained admission is genuinely
    /// legacy_chat, never a malformed/missing private original or a fallback.
    pub(crate) async fn verify_private_live_completion(
        tx: &Transaction<'_>,
        completion: &crate::global_monty_driver::SettledInternalCompletion,
    ) -> Result<Option<PrivateCompletionOriginal>, AgentLoopDriverError> {
        // This public-to-crate operation is authentication only. Its actual
        // consumers read existing settled originals in READ ONLY snapshots.
        Self::verify_private_completion(tx, completion, InvocationRefsRead::Snapshot).await
    }

    async fn verify_private_completion(
        tx: &Transaction<'_>,
        completion: &crate::global_monty_driver::SettledInternalCompletion,
        access: InvocationRefsRead,
    ) -> Result<Option<PrivateCompletionOriginal>, AgentLoopDriverError> {
        let invalid = || failed("private_completion_original_invalid");
        let attempt = completion.attempt();
        let row = tx
            .query_opt(
                "SELECT origin_kind,private_work_id,private_execution_context,
            runner_id,claim_checksum,admission_key,phase,outcome,private_request_checksum
            FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&attempt.run_id.as_uuid()],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .ok_or_else(invalid)?;
        if row.get::<_, &str>(0) == "legacy_chat" {
            return Ok(None);
        }
        if row.get::<_, &str>(0) != "private_work"
            || row.get::<_, &str>(6) != "settled"
            || row.get::<_, Uuid>(3) != attempt.runner_id.as_uuid()
        {
            return Err(invalid());
        }
        let lease = serde_json::to_vec(&attempt.lease_token).map_err(|_| invalid())?;
        let claim_checksum: [u8; 32] = Sha256::digest(lease).into();
        if row.get::<_, Vec<u8>>(4).as_slice() != claim_checksum {
            return Err(invalid());
        }
        let context: brassclaw_turns::private_work::PrivateExecutionContext =
            serde_json::from_value(row.get(2)).map_err(|_| invalid())?;
        let work_id: Uuid = row.get(1);
        let state = brassclaw_turns::PgTurnStateStore::read_private_work_state_in_transaction(
            tx,
            context.tenant_id.as_str(),
            work_id,
        )
        .await
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
        if state.status != brassclaw_turns::private_work::PrivateWorkStatus::Settled
            || state.request.context() != &context
            || state.request.run_id() != attempt.run_id
            || row.get::<_, Vec<u8>>(8).as_slice() != state.request_checksum
        {
            return Err(invalid());
        }
        let original=tx.query_one("SELECT p.definition_bytes,p.checksum,t.terminal_bytes,t.checksum,t.execution_report_bytes
            FROM brassclaw_monty_private_preparations p JOIN brassclaw_monty_private_terminals t USING(work_id)
            WHERE p.work_id=$1 AND t.run_id=$2",&[&work_id,&attempt.run_id.as_uuid()])
            .await.map_err(|_|failed("monty_admission_database_failed"))?;
        let definition_checksum: [u8; 32] = original
            .get::<_, Vec<u8>>(1)
            .try_into()
            .map_err(|_| invalid())?;
        let definition = crate::private_work_definition::PrivateWorkDefinition::restore(
            tx,
            &state.request,
            state.request_checksum,
            original.get::<_, &str>(0),
            definition_checksum,
        )
        .await
        .map_err(|_| invalid())?;
        if definition.root().source_checksum() != completion.receipt().root.source_checksum() {
            return Err(invalid());
        }
        let terminal_checksum: [u8; 32] = original
            .get::<_, Vec<u8>>(3)
            .try_into()
            .map_err(|_| invalid())?;
        if Some(terminal_checksum) != state.terminal_checksum {
            return Err(invalid());
        }
        let report_bytes = original.get::<_, Option<&str>>(4).ok_or_else(invalid)?;
        let report: Value = serde_json::from_str(report_bytes).map_err(|_| invalid())?;
        let outcome: Value = row.get::<_, Option<Value>>(7).ok_or_else(invalid)?;
        if outcome.get("execution") != Some(&report) {
            return Err(invalid());
        }
        let address = terminal::Address {
            work_id,
            run_id: attempt.run_id.as_uuid(),
            claim_generation: 1,
            claim_checksum,
            request_checksum: state.request_checksum,
            definition_checksum,
            admission_ref: Sha256::digest(row.get::<_, Vec<u8>>(5)).into(),
            cancellation_revision: state.cancellation_revision,
        };
        let native = terminal::Terminal::native(
            address,
            completion.receipt(),
            &report,
            Self::read_private_invocation_refs(tx, attempt.run_id, access).await?,
        )?;
        if native.checksum() != terminal_checksum
            || native.bytes() != original.get::<_, &str>(2)
            || native.report_bytes() != Some(report_bytes)
        {
            return Err(invalid());
        }
        Ok(Some(PrivateCompletionOriginal {
            request: state.request,
            definition,
        }))
    }
}

pub(crate) use terminal::HistoricalOutcome as PrivateOriginalTerminalStatus;
impl PgMontyAdmission {
    /// Authenticate historical failed/successful terminal metadata without
    /// minting a live claim, native completion, delivery or replay permission.
    pub(crate) async fn read_private_terminal_status(
        tx: &Transaction<'_>,
        work_id: Uuid,
        context: &brassclaw_turns::private_work::PrivateExecutionContext,
    ) -> Result<Option<PrivateOriginalTerminalStatus>, AgentLoopDriverError> {
        let invalid = || failed("private_terminal_original_invalid");
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
        let Some(row)=tx.query_opt("SELECT p.definition_bytes,p.checksum,t.terminal_bytes,t.checksum,t.execution_report_bytes,
            a.runner_id,a.claim_checksum,a.admission_key,a.private_claim_generation,a.private_request_checksum,
            a.outcome,a.private_execution_context,a.native_actor,w.private_definition_checksum,w.private_claim_checksum,
            w.private_claim_generation,w.private_runner_id,w.private_admission_run
            FROM brassclaw_monty_private_terminals t
            JOIN brassclaw_monty_private_preparations p USING(work_id)
            JOIN brassclaw_monty_task_admissions a ON a.run_id=t.run_id AND a.private_work_id=t.work_id
            JOIN brassclaw_turns w ON w.work_id=t.work_id AND w.execution_run_id=t.run_id
            WHERE t.work_id=$1 AND t.run_id=$2 AND a.origin_kind='private_work' AND a.phase='settled'",
            &[&work_id,&state.request.run_id().as_uuid()]).await.map_err(|_|failed("monty_admission_database_failed"))? else {
                if state.status==brassclaw_turns::private_work::PrivateWorkStatus::Settled {return Err(invalid());}
                return Ok(None);
            };
        if state.status != brassclaw_turns::private_work::PrivateWorkStatus::Settled {
            return Err(invalid());
        }
        let definition_checksum: [u8; 32] =
            row.get::<_, Vec<u8>>(1).try_into().map_err(|_| invalid())?;
        let definition = crate::private_work_definition::PrivateWorkDefinition::restore(
            tx,
            &state.request,
            state.request_checksum,
            row.get(0),
            definition_checksum,
        )
        .await
        .map_err(|_| invalid())?;
        let terminal_checksum: [u8; 32] =
            row.get::<_, Vec<u8>>(3).try_into().map_err(|_| invalid())?;
        let outcome: Value = row.get::<_, Option<Value>>(10).ok_or_else(invalid)?;
        let (address, status, refs) = terminal::Terminal::read_outcome(
            row.get(2),
            terminal_checksum,
            row.get(4),
            &outcome,
            definition.root().source_checksum(),
        )?;
        if status == PrivateOriginalTerminalStatus::Successful {
            Self::validate_private_result_reference(&outcome, state.request.run_id())?;
        }
        let expected = terminal::Address {
            work_id,
            run_id: state.request.run_id().as_uuid(),
            claim_generation: 1,
            claim_checksum: row.get::<_, Vec<u8>>(6).try_into().map_err(|_| invalid())?,
            request_checksum: state.request_checksum,
            definition_checksum,
            admission_ref: Sha256::digest(row.get::<_, Vec<u8>>(7)).into(),
            cancellation_revision: state.cancellation_revision,
        };
        if address != expected
            || state.terminal_checksum != Some(terminal_checksum)
            || row.get::<_, i64>(8) != 1
            || row.get::<_, i64>(15) != 1
            || row.get::<_, Vec<u8>>(9).as_slice() != state.request_checksum
            || row.get::<_, Value>(11) != serde_json::to_value(context).map_err(|_| invalid())?
            || row.get::<_, Value>(12)
                != serde_json::to_value(&context.actor).map_err(|_| invalid())?
            || row.get::<_, Vec<u8>>(13).as_slice() != definition.checksum()
            || row.get::<_, Vec<u8>>(14).as_slice() != expected.claim_checksum
            || row.get::<_, Uuid>(16) != row.get::<_, Uuid>(5)
            || row.get::<_, Uuid>(17) != state.request.run_id().as_uuid()
            || Self::read_private_invocation_refs(
                tx,
                state.request.run_id(),
                InvocationRefsRead::Snapshot,
            )
            .await?
                != refs
        {
            return Err(invalid());
        }
        Ok(Some(status))
    }
}
