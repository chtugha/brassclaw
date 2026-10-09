//! Instance-service task handoff candidate for the Phase 3a production cutover.
//!
//! The ABI/resource/catalogue acceptance gate still controls startup wiring.
//! This adapter neither starts a VM nor loads a legacy UUID engine Thread.
//! Python owns all selection and sequencing through the supplied task ports.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::monty_attempt_retention::{AttemptRetentionLease, AttemptRetentionRegistry};
use async_trait::async_trait;
use brassclaw_monty_host::service::{
    ServiceClient, TaskControl, TaskInput, TaskOutcome, TaskPorts, TaskReceipt,
};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_threads::SessionThreadService;
use brassclaw_turns::{
    LoopCompleted, LoopCompletionKind, LoopExit, LoopExitId, LoopMessageRef,
    run_profile::{AgentLoopDriverError, MontyTaskAttempt, MontyTaskHandoff, MontyTurnDriverPort},
};
use futures::{
    FutureExt,
    future::{BoxFuture, Shared},
};
use tokio::sync::Notify;

/// Last complete acknowledged policy. Capturing it grants no dispatch authority.
#[derive(Clone, Copy)]
pub(crate) struct CancellationAckSettings {
    pub(crate) revision: u64,
    pub(crate) timeout: Duration,
}
impl CancellationAckSettings {
    pub(crate) fn validate(self) -> Result<Self, AgentLoopDriverError> {
        if self.revision == 0
            || self.timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(self.timeout)
                .is_none()
        {
            return Err(invalid("invalid Monty cancellation acknowledgement policy"));
        }
        Ok(self)
    }
}
pub(crate) trait CancellationAckSource: Send + Sync {
    fn current(&self) -> Result<CancellationAckSettings, AgentLoopDriverError>;
}

/// Builds the exact admitted ports and catalogue snapshot, without executing a
/// Tool, resolving intent or selecting a Recipe. Those operations are requested
/// by the already-running Python orchestrator after admission.
#[async_trait]
pub(crate) trait GlobalTaskPortsFactory: Send + Sync {
    fn retention_registry(&self) -> Arc<AttemptRetentionRegistry>;
    fn cancellation_acknowledgement(&self)
    -> Result<CancellationAckSettings, AgentLoopDriverError>;
    async fn build(
        &self,
        host: Arc<MontyTaskHost>,
        input: &TaskInput,
    ) -> Result<Arc<dyn TaskPorts>, AgentLoopDriverError>;

    /// Persist the actual service settlement under this exact admitted attempt.
    /// This follows root/child/host-future acknowledgement, never finish_task's
    /// request alone. Failed persistence leaves the attempt retained for recovery.
    async fn settle(
        &self,
        host: Arc<MontyTaskHost>,
        receipt: Arc<TaskReceipt>,
    ) -> Result<(), AgentLoopDriverError>;
}

enum Phase {
    Preparing,
    PreparationStopped,
    Submitted(TaskControl),
}
struct AttemptState {
    cancelled: bool,
    phase: Phase,
}
struct Attempt {
    host: Arc<MontyTaskHost>,
    retention: Arc<AttemptRetentionLease>,
    state: Mutex<AttemptState>,
    changed: Notify,
    settlement: Mutex<Option<Settlement>>,
}

struct Settlement {
    receipt: Arc<TaskReceipt>,
    acknowledgement: Shared<BoxFuture<'static, Result<(), AgentLoopDriverError>>>,
}

/// Retains attempts abandoned by runner waiters until actual service settlement.
/// An unresolved service failure stays registered for trusted reconciliation;
/// it never authorizes resubmission or a Rust agent-loop fallback.
pub(crate) struct GlobalMontyDriver {
    service: ServiceClient,
    threads: Arc<dyn SessionThreadService>,
    ports: Arc<dyn GlobalTaskPortsFactory>,
    attempts: Mutex<HashMap<MontyTaskAttempt, Arc<Attempt>>>,
    retention: Arc<AttemptRetentionRegistry>,
}

/// Private host ownership plus its actual service receipt for reconciliation.
// Control retains the actual TaskPorts, including child failures and Tool answers.
// Transferring only the host/receipt would discard that reconciliation evidence.
#[cfg(test)]
pub(crate) type MontySettlement = (
    Arc<MontyTaskHost>,
    Arc<TaskReceipt>,
    TaskControl,
    Arc<AttemptRetentionLease>,
);

impl GlobalMontyDriver {
    /// Trusted fixture inspection of the original host, never a reconstructed
    /// claim or permission. Used by real cancellation/policy acceptance.
    #[cfg(test)]
    pub(crate) fn retained_host_for_run(
        &self,
        run: brassclaw_turns::TurnRunId,
    ) -> Result<Option<Arc<MontyTaskHost>>, AgentLoopDriverError> {
        let attempts = self
            .attempts
            .lock()
            .map_err(|_| failed("monty_attempt_registry_failed"))?;
        Ok(attempts
            .iter()
            .find(|(attempt, _)| attempt.run_id == run)
            .map(|(_, entry)| entry.host.clone()))
    }
    pub(crate) fn new(
        service: ServiceClient,
        threads: Arc<dyn SessionThreadService>,
        ports: Arc<dyn GlobalTaskPortsFactory>,
    ) -> Result<Self, AgentLoopDriverError> {
        let retention = ports.retention_registry();
        if !retention.bound_to(&service) {
            return Err(invalid(
                "Monty driver and retention owner identify different instances",
            ));
        }
        Ok(Self {
            service,
            threads,
            ports,
            attempts: Mutex::new(HashMap::new()),
            retention,
        })
    }

    fn register(&self, host: Arc<MontyTaskHost>) -> Result<Arc<Attempt>, AgentLoopDriverError> {
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| failed("monty_attempt_registry_failed"))?;
        if attempts.contains_key(&host.attempt()) {
            return Err(invalid("Monty attempt is already registered"));
        }
        let retention = self.retention.retain(&host)?;
        let entry = Arc::new(Attempt {
            host,
            retention,
            state: Mutex::new(AttemptState {
                cancelled: false,
                phase: Phase::Preparing,
            }),
            changed: Notify::new(),
            settlement: Mutex::new(None),
        });
        attempts.insert(entry.host.attempt(), entry.clone());
        Ok(entry)
    }

    fn remove(&self, attempt: MontyTaskAttempt) -> Result<(), AgentLoopDriverError> {
        self.attempts
            .lock()
            .map_err(|_| failed("monty_attempt_registry_failed"))?
            .remove(&attempt);
        Ok(())
    }

    async fn settle_attempt(
        &self,
        entry: &Arc<Attempt>,
        receipt: Arc<TaskReceipt>,
    ) -> Result<(), AgentLoopDriverError> {
        let acknowledgement = {
            let mut retained = entry
                .settlement
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            if let Some(previous) = retained.as_ref()
                && !Arc::ptr_eq(&previous.receipt, &receipt)
            {
                return Err(failed("monty_task_settlement_conflict"));
            }
            // Concurrent drive/stop waiters share one durable acknowledgement.
            // Dropping a waiter retains the future; a later stop resumes it.
            // A completed persistence failure may retry only the same receipt
            // and private admission, never task execution or external effects.
            if retained
                .as_ref()
                .is_none_or(|previous| previous.acknowledgement.peek().is_some_and(Result::is_err))
            {
                let ports = self.ports.clone();
                let host = entry.host.clone();
                let original = receipt.clone();
                *retained = Some(Settlement {
                    receipt,
                    acknowledgement: async move { ports.settle(host, original).await }
                        .boxed()
                        .shared(),
                });
            }
            retained
                .as_ref()
                .ok_or_else(|| failed("monty_attempt_state_failed"))?
                .acknowledgement
                .clone()
        };
        acknowledgement.await
    }

    /// Trusted supervisor receipt for a dropped turn waiter or failed attempt.
    /// The service's real outcome and late answers stay attached to the host.
    /// Removing this entry transfers reconciliation responsibility to the caller.
    #[cfg(test)]
    pub(crate) fn take_settlement(
        &self,
        attempt: MontyTaskAttempt,
    ) -> Result<Option<MontySettlement>, AgentLoopDriverError> {
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| failed("monty_attempt_registry_failed"))?;
        let entry = attempts.get(&attempt).cloned();
        let Some(entry) = entry else {
            return Ok(None);
        };
        let control = {
            let state = entry
                .state
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            match &state.phase {
                Phase::Submitted(control) => control.clone(),
                Phase::PreparationStopped => {
                    drop(state);
                    attempts.remove(&attempt);
                    return Ok(None);
                }
                Phase::Preparing => {
                    return Err(AgentLoopDriverError::Unavailable {
                        reason: "Monty task preparation is still active".into(),
                    });
                }
            }
        };
        let receipt = control
            .receipt()
            .ok_or_else(|| AgentLoopDriverError::Unavailable {
                reason: "Monty attempt has not settled".into(),
            })?
            .map_err(|_| failed("monty_service_reconciliation_required"))?;
        {
            let retained = entry
                .settlement
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            let acknowledgement = retained
                .as_ref()
                .filter(|settlement| Arc::ptr_eq(&settlement.receipt, &receipt))
                .and_then(|settlement| settlement.acknowledgement.peek());
            match acknowledgement {
                Some(Ok(())) => {}
                Some(Err(error)) => return Err(error.clone()),
                None => {
                    return Err(AgentLoopDriverError::Unavailable {
                        reason: "Monty durable settlement is still pending".into(),
                    });
                }
            }
        }
        attempts.remove(&attempt);
        Ok(Some((
            entry.host.clone(),
            receipt,
            control,
            entry.retention.clone(),
        )))
    }
}

/// A runner dropping its drive future fences only its own task. Submitted host
/// futures remain owned by the instance service and the registry retains control.
struct DriveGuard(Arc<Attempt>);
impl Drop for DriveGuard {
    fn drop(&mut self) {
        self.0.host.fence_dispatch();
        if let Ok(mut state) = self.0.state.lock() {
            state.cancelled = true;
            match &state.phase {
                Phase::Preparing => state.phase = Phase::PreparationStopped,
                Phase::Submitted(control) => control.cancel(),
                Phase::PreparationStopped => {}
            }
        }
        self.0.changed.notify_waiters();
    }
}

#[async_trait]
impl MontyTurnDriverPort for GlobalMontyDriver {
    async fn drive_turn(
        &self,
        handoff: MontyTaskHandoff,
    ) -> Result<LoopExit, AgentLoopDriverError> {
        let host = Arc::new(MontyTaskHost::new(handoff));
        let entry = self.register(host.clone())?;
        let guard = DriveGuard(entry.clone());
        let context = host.run_context();
        let input = match crate::monty_task_input::load_monty_task_input(
            self.threads.as_ref(),
            &context.scope,
            context.accepted_message_ref.as_ref(),
            context.turn_id,
            context.run_id,
        )
        .await
        {
            Ok(input) => input,
            Err(error) => {
                drop(guard);
                self.remove(host.attempt())?;
                return Err(error);
            }
        };
        let Some(user_input) = input.message.content else {
            drop(guard);
            self.remove(host.attempt())?;
            return Err(failed("monty_admitted_input_invalid"));
        };
        // Identity, user content and complete eligible history are typed values,
        // never Python source. Technical transport limits reject oversized work;
        // they must not silently turn history into an empty or truncated list.
        let input = TaskInput {
            conversation_id: context.thread_id.to_string(),
            message_id: input.message.message_id.to_string(),
            turn_id: context.turn_id.to_string(),
            run_id: context.run_id.to_string(),
            user_input,
            history: input
                .prior_context
                .messages
                .into_iter()
                .map(|message| {
                    serde_json::json!({
                        "message_id": message.message_id, "sequence": message.sequence,
                        "kind": message.kind, "content": message.content,
                    })
                })
                .collect(),
        };
        // History preparation may await. Keep the shared private credit active
        // before constructing ports or retaining an admission address.
        if let Err(error) = entry.retention.ensure_active(&host) {
            drop(guard);
            self.remove(host.attempt())?;
            return Err(error);
        }
        let ports = match self.ports.build(host.clone(), &input).await {
            Ok(ports) => ports,
            Err(error) => {
                drop(guard);
                self.remove(host.attempt())?;
                return Err(error);
            }
        };
        {
            let state = entry
                .state
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            if state.cancelled {
                drop(state);
                drop(guard);
                self.remove(host.attempt())?;
                return Err(failed("monty_task_cancelled"));
            }
        }
        let ticket = match self.service.submit_when_available(input, ports).await {
            Ok(ticket) => ticket,
            Err(_) => {
                // No VM admission was issued. Release the registry entry just
                // as for failed input/port preparation; do not leak capacity.
                drop(guard);
                self.remove(host.attempt())?;
                return Err(failed("monty_task_admission_failed"));
            }
        };
        {
            let mut state = entry
                .state
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            // Always retain an accepted control, including a cancellation racing
            // with queue admission. Settlement must wait for its actual receipt.
            state.phase = Phase::Submitted(ticket.control());
            if state.cancelled {
                ticket.control().cancel();
            }
            entry.changed.notify_waiters();
        }
        let receipt = ticket
            .wait()
            .await
            .map_err(|_| failed("monty_service_reconciliation_required"))?;
        match &receipt.outcome {
            TaskOutcome::InternalCompleted { .. } => Err(failed("monty_reply_reference_invalid")),
            TaskOutcome::Completed { reply_ref } => {
                let reference = LoopMessageRef::new(reply_ref.clone())
                    .map_err(|_| failed("monty_reply_reference_invalid"))?;
                host.published_reply_content(&reference)
                    .map_err(|_| failed("monty_reply_not_published"))?;
                let exit = LoopExit::Completed(LoopCompleted {
                    completion_kind: LoopCompletionKind::FinalReply,
                    reply_message_refs: vec![reference],
                    result_refs: Vec::new(),
                    final_checkpoint_id: None,
                    usage_summary_ref: None,
                    exit_id: LoopExitId::new(format!("exit:{}-completed", context.run_id))
                        .map_err(|_| failed("monty_exit_reference_invalid"))?,
                });
                // A successful task cannot have a fenced late answer. If this
                // invariant fails, retain the receipt instead of dropping evidence.
                if !receipt.withheld.is_empty() || host.has_withheld_results() {
                    return Err(failed("monty_service_reconciliation_required"));
                }
                self.settle_attempt(&entry, receipt.clone()).await?;
                drop(guard);
                self.remove(host.attempt())?;
                Ok(exit)
            }
            TaskOutcome::Failed { reason_kind } => {
                self.settle_attempt(&entry, receipt.clone()).await?;
                Err(failed(reason_kind))
            }
        }
    }

    async fn stop_attempt(&self, attempt: MontyTaskAttempt) -> Result<(), AgentLoopDriverError> {
        let entry = self
            .attempts
            .lock()
            .map_err(|_| failed("monty_attempt_registry_failed"))?
            .get(&attempt)
            .cloned();
        let Some(entry) = entry else {
            return Ok(());
        };
        entry.host.fence_dispatch();
        // Fence and request cancellation before policy lookup or any await.
        // Even an unavailable policy must not leave a submitted task running.
        {
            let mut state = entry
                .state
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            state.cancelled = true;
            if let Phase::Submitted(control) = &state.phase {
                control.cancel();
            }
        }
        entry.changed.notify_waiters();
        let policy = self.ports.cancellation_acknowledgement()?.validate()?;
        tokio::time::timeout(policy.timeout, async {
            loop {
                let changed = entry.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                let control = {
                    let mut state = entry
                        .state
                        .lock()
                        .map_err(|_| failed("monty_attempt_state_failed"))?;
                    state.cancelled = true;
                    match &state.phase {
                        Phase::PreparationStopped => return Ok(()),
                        Phase::Submitted(control) => Some(control.clone()),
                        Phase::Preparing => None,
                    }
                };
                if let Some(control) = control {
                    control.cancel();
                    let receipt = control
                        .wait()
                        .await
                        .map_err(|_| failed("monty_service_reconciliation_required"))?;
                    self.settle_attempt(&entry, receipt).await?;
                    return Ok(());
                }
                changed.await;
            }
        })
        .await
        .map_err(|_| AgentLoopDriverError::Unavailable {
            reason: "Monty attempt did not acknowledge settlement before deadline".into(),
        })?
    }
}

fn invalid(reason: &str) -> AgentLoopDriverError {
    AgentLoopDriverError::InvalidRequest {
        reason: reason.into(),
    }
}
fn failed(reason: &str) -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: reason.into(),
    }
}
