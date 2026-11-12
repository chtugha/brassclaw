//! Original contained admission custody before its first possible write. The
//! pending reference is immutable metadata, never an execution/approval grant.
use super::*;
use brassclaw_threads::{AcceptedInboundMessage, ReplayAcceptedInboundMessageRequest};
use brassclaw_turns::{
    CancelRunRequest, GetRunStateRequest, TurnSpawnTreeStateStore, TurnStateStore, TurnStatus,
    runner::CancelRunCompletionRequest,
};
use sha2::{Digest, Sha256};

pub(crate) struct PreparedAdmission {
    pool: Arc<PgPool>,
    identity: String,
    answer: String,
    pub(super) conversation: ThreadId,
    scope: ThreadScope,
    turn_scope: TurnScope,
    actor: UserId,
    pub(super) coordinator: Arc<DefaultTurnCoordinator<PgTurnStateStore>>,
    state: Arc<PgTurnStateStore>,
    pub(super) attempt: MontyTaskAttempt,
    pub(super) submit_key: IdempotencyKey,
    pub(super) inbound_event: String,
    binding: Option<(Uuid, String)>,
    retained: bool,
    called: bool,
    pub(super) submit_started: bool,
    pub(super) accepted: Option<AcceptedInboundMessage>,
    pub(super) admission: Option<Arc<PgMontyAdmission>>,
    pub(super) handed_off: bool,
    cancel_started: bool,
    completion_started: bool,
    settlement_started: bool,
    closed_turn: Option<brassclaw_turns::TurnRunRecord>,
}
impl PreparedAdmission {
    pub(crate) async fn new(
        pool: Arc<PgPool>,
        identity: &str,
        answer: &str,
    ) -> Result<Self, RebornBuildError> {
        let tenant = TenantId::new(NAMESPACE).map_err(invalid)?;
        let agent = AgentId::new(identity).map_err(invalid)?;
        let actor = UserId::new(INSTALLATION_ACTOR).map_err(invalid)?;
        let project = Some(ProjectId::new("contained").map_err(invalid)?);
        let conversation =
            ThreadId::new(format!("{identity}-{}", Uuid::new_v4())).map_err(invalid)?;
        let scope = ThreadScope {
            tenant_id: tenant.clone(),
            agent_id: agent.clone(),
            project_id: project.clone(),
            owner_user_id: None,
        };
        let turn_scope = TurnScope::new(tenant.clone(), Some(agent), project, conversation.clone());
        let state = Arc::new(PgTurnStateStore::new(pool.clone(), tenant.as_str()));
        let coordinator = Arc::new(DefaultTurnCoordinator::new(state.clone()));
        // Existing coordinator allocation, with its actual prepared-scope map.
        // This creates no Turn, claim, admission, VM task or database effect.
        let run_id = coordinator
            .prepare_turn(turn_scope.clone())
            .await
            .map_err(invalid)?;
        Ok(Self {
            pool,
            identity: identity.into(),
            answer: answer.into(),
            conversation,
            scope,
            turn_scope,
            actor,
            coordinator,
            state,
            attempt: MontyTaskAttempt {
                run_id,
                runner_id: TurnRunnerId::new(),
                lease_token: TurnLeaseToken::new(),
            },
            submit_key: IdempotencyKey::new(Uuid::new_v4().to_string()).map_err(invalid)?,
            inbound_event: Uuid::new_v4().to_string(),
            binding: None,
            retained: false,
            called: false,
            submit_started: false,
            accepted: None,
            admission: None,
            handed_off: false,
            cancel_started: false,
            completion_started: false,
            settlement_started: false,
            closed_turn: None,
        })
    }

    pub(crate) async fn retain(
        &mut self,
        storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
        id: Uuid,
        parent: Value,
    ) -> Result<(), RebornBuildError> {
        if self.binding.is_some() {
            return Err(invalid(
                "bootstrap original admission binding already selected",
            ));
        }
        let claim_checksum = hex::encode(Sha256::digest(
            serde_json::to_vec(&self.attempt.lease_token).map_err(invalid)?,
        ));
        let report = json!({"format":"bootstrap-admission-original/1", "parent":parent,
            "scope":self.turn_scope,"conversation_id":self.conversation,"run_id":self.attempt.run_id,
            "runner_id":self.attempt.runner_id,"claim_checksum":claim_checksum,
            "inbound_event_id":self.inbound_event,
            "submit_checksum":hex::encode(Sha256::digest(self.submit_key.as_str().as_bytes())),
            "input_checksum":hex::encode(Sha256::digest(self.answer.as_bytes())),
            "completion":"unresolved_until_original_close"});
        let bytes = crate::bootstrap_evidence::system_record(
            id,
            "behavior",
            false,
            &crate::bootstrap_evidence::checksum(&report.to_string()),
            &[],
            report,
        );
        // Retain the selected reference even if this write's acknowledgement is
        // lost. An uncertain bind never permits the first admission write.
        self.binding = Some((id, bytes.clone()));
        crate::bootstrap_evidence::io::retain_new_intent(&self.pool, storage, id, &bytes).await?;
        self.retained = true;
        Ok(())
    }

    pub(crate) fn reference(&self) -> Value {
        self.binding.as_ref().map_or(Value::Null, |(id, bytes)| {
            json!({
                "id":id,"checksum":crate::bootstrap_evidence::checksum(bytes),
            })
        })
    }

    pub(crate) async fn admit(
        &mut self,
        trace: Option<&AdmissionTrace>,
    ) -> Result<Admitted, RebornBuildError> {
        if !self.retained || self.called {
            return Err(invalid(
                "bootstrap original admission cannot be started or replayed",
            ));
        }
        self.called = true;
        let pool = self.pool.clone();
        let identity = self.identity.clone();
        let answer = self.answer.clone();
        prepare_admission(
            pool,
            &identity,
            &answer,
            AdmissionMode::Contained,
            trace,
            Some(self),
        )
        .await
    }

    /// Explicit reconciliation of this exact original only. Dropping a caller
    /// does not spawn unowned work: its pre-write pending reference remains a
    /// visible unresolved original until this owner can confirm its close.
    pub(crate) async fn close(
        &mut self,
        storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    ) -> Result<Value, RebornBuildError> {
        if self.handed_off || !self.called {
            return Err(invalid(
                "bootstrap admission close requires an interrupted original",
            ));
        }
        let (id, _) = self
            .binding
            .as_ref()
            .ok_or_else(|| invalid("bootstrap original admission binding absent"))?;
        let id = *id;
        let closed = crate::bootstrap_evidence::io::bounded_for(
            storage,
            "bootstrap_original_admission_close",
            id,
            self.close_original(),
        )
        .await?;
        let bytes = crate::bootstrap_evidence::system_record(
            Uuid::new_v5(&id, b"original-close"),
            "behavior",
            false,
            &crate::bootstrap_evidence::checksum(&closed.to_string()),
            &[],
            closed.clone(),
        );
        crate::bootstrap_evidence::io::retain_record(
            &self.pool,
            storage,
            Uuid::new_v5(&id, b"original-close"),
            &bytes,
        )
        .await?;
        Ok(closed)
    }

    pub(crate) fn closed_turn(&self) -> Option<&brassclaw_turns::TurnRunRecord> {
        self.closed_turn.as_ref()
    }

    pub(crate) async fn failure(
        &mut self,
        storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
        error: RebornBuildError,
    ) -> RebornBuildError {
        let close = self.close(storage).await;
        invalid(format!(
            "{error}; original admission binding: {}; original cleanup: {}",
            self.reference(),
            match close {
                Ok(observed) => observed.to_string(),
                Err(error) => format!("unresolved ({error})"),
            }
        ))
    }

    async fn close_original(&mut self) -> Result<Value, RebornBuildError> {
        let threads = PgSessionThreadService::new(self.pool.clone(), NAMESPACE);
        let receipt = threads
            .replay_accepted_inbound_message(ReplayAcceptedInboundMessageRequest {
                scope: self.scope.clone(),
                actor_id: self.actor.to_string(),
                source_binding_id: "bootstrap-internal".into(),
                external_event_id: self.inbound_event.clone(),
            })
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("bootstrap original inbound outcome unresolved"))?;
        if receipt.thread_id != self.conversation
            || self
                .accepted
                .as_ref()
                .is_some_and(|accepted| accepted.message_id != receipt.message_id)
        {
            return Err(invalid("bootstrap original inbound identity differs"));
        }
        let expected_input =
            AcceptedMessageRef::new(format!("msg:{}", receipt.message_id)).map_err(invalid)?;
        let original = self
            .state
            .get_run_record(&self.turn_scope, self.attempt.run_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("bootstrap original submit outcome unresolved"))?;
        self.require_run(&original, &expected_input)?;
        self.require_no_invocations().await?;
        let current = self
            .state
            .get_run_state(GetRunStateRequest {
                scope: self.turn_scope.clone(),
                run_id: self.attempt.run_id,
            })
            .await
            .map_err(invalid)?;
        if current.actor != Some(TurnActor::new(self.actor.clone())) {
            return Err(invalid("bootstrap original actor differs"));
        }
        if !original.status.is_terminal() && original.status != TurnStatus::CancelRequested {
            if self.cancel_started {
                return Err(invalid(
                    "bootstrap original cancellation outcome unresolved",
                ));
            }
            self.cancel_started = true;
            self.state
                .request_cancel(CancelRunRequest {
                    scope: self.turn_scope.clone(),
                    actor: TurnActor::new(self.actor.clone()),
                    run_id: self.attempt.run_id,
                    reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
                    idempotency_key: IdempotencyKey::new(format!(
                        "bootstrap-original-close:{}",
                        self.attempt.run_id
                    ))
                    .map_err(invalid)?,
                })
                .await
                .map_err(invalid)?;
        }
        let cancelled = self
            .state
            .get_run_record(&self.turn_scope, self.attempt.run_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("bootstrap original cancelled run absent"))?;
        self.require_run(&cancelled, &expected_input)?;
        if cancelled.status == TurnStatus::CancelRequested {
            if self.completion_started {
                return Err(invalid("bootstrap original cancel completion unresolved"));
            }
            self.completion_started = true;
            self.state
                .cancel_run(CancelRunCompletionRequest {
                    run_id: self.attempt.run_id,
                    runner_id: self.attempt.runner_id,
                    lease_token: self.attempt.lease_token,
                })
                .await
                .map_err(invalid)?;
        }
        let terminal = self
            .state
            .get_run_record(&self.turn_scope, self.attempt.run_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("bootstrap original terminal run absent"))?;
        self.require_run(&terminal, &expected_input)?;
        if terminal.status != TurnStatus::Cancelled
            || terminal.runner_id.is_some()
            || terminal.lease_token.is_some()
            || terminal.lease_expires_at.is_some()
            || terminal.native_auth_wait.is_some()
        {
            return Err(invalid(
                "bootstrap original cancellation is not acknowledged",
            ));
        }
        // The acknowledged Turn close fences any provisional reservation/start
        // via their existing final claim lock. No new reservation is permitted.
        let phase = match &self.admission {
            Some(admission) => {
                let phase = admission.read_original_phase().await.map_err(invalid)?;
                if matches!(
                    phase,
                    Some(crate::pg_monty_admission::OriginalAdmissionPhase::Open)
                ) {
                    if self.settlement_started {
                        return Err(invalid("bootstrap original journal close unresolved"));
                    }
                    self.settlement_started = true;
                    admission
                        .settle(json!({"status":"failed","reason_kind":"task_cancelled"}))
                        .await
                        .map_err(invalid)?;
                }
                match admission.read_original_phase().await.map_err(invalid)? {
                    None => "absent",
                    Some(crate::pg_monty_admission::OriginalAdmissionPhase::Cancelled) => {
                        "cancelled"
                    }
                    _ => return Err(invalid("bootstrap original journal close not acknowledged")),
                }
            }
            None => "not_prepared",
        };
        self.require_no_invocations().await?;
        self.closed_turn = Some(terminal);
        Ok(
            json!({"format":"bootstrap-admission-close/1","original":self.reference(),"run_id":self.attempt.run_id,
            "status":"cancelled","admission":phase,"execution_handed_off":false}),
        )
    }

    async fn require_no_invocations(&self) -> Result<(), RebornBuildError> {
        let client = self.pool.get().await.map_err(invalid)?;
        let effects: i64 = client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&self.attempt.run_id.as_uuid()],
            )
            .await
            .map_err(invalid)?
            .get(0);
        if effects != 0 {
            return Err(invalid(
                "interrupted bootstrap original has native invocation custody",
            ));
        }
        Ok(())
    }

    fn require_run(
        &self,
        run: &brassclaw_turns::TurnRunRecord,
        input: &AcceptedMessageRef,
    ) -> Result<(), RebornBuildError> {
        let claim_coherent = match run.status {
            TurnStatus::Queued => {
                run.claim_count == 0 && run.runner_id.is_none() && run.lease_token.is_none()
            }
            TurnStatus::Running | TurnStatus::CancelRequested => {
                run.claim_count == 1
                    && run.runner_id == Some(self.attempt.runner_id)
                    && run.lease_token == Some(self.attempt.lease_token)
            }
            TurnStatus::Cancelled => run.runner_id.is_none() && run.lease_token.is_none(),
            _ => false,
        };
        if !claim_coherent
            || !self.submit_started
            || run.run_id != self.attempt.run_id
            || run.scope != self.turn_scope
            || run.accepted_message_ref != *input
            || run.source_binding_ref.as_str() != "bootstrap-internal"
            || run.reply_target_binding_ref.as_str() != "bootstrap-internal"
            || run.parent_run_id.is_some()
            || run.claim_count > 1
            || run.native_auth_wait.is_some()
            || run
                .runner_id
                .is_some_and(|runner| runner != self.attempt.runner_id)
            || run
                .lease_token
                .is_some_and(|lease| lease != self.attempt.lease_token)
        {
            return Err(invalid("bootstrap original run or claim identity differs"));
        }
        Ok(())
    }
}

#[cfg(all(test, unix))]
#[path = "bootstrap_admission_original_tests.rs"]
mod tests;
#[cfg(all(test, unix))]
pub(super) async fn before_phase(original: Option<&PreparedAdmission>, phase: &'static str) {
    if original.is_some() {
        tests::before_phase(phase).await;
    }
}
