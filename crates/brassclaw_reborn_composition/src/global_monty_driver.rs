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
use tokio::sync::Notify;

/// Builds the exact admitted ports and catalogue snapshot, without executing a
/// Tool, resolving intent or selecting a Recipe. Those operations are requested
/// by the already-running Python orchestrator after admission.
#[async_trait]
pub(crate) trait GlobalTaskPortsFactory: Send + Sync {
    async fn build(
        &self,
        host: Arc<MontyTaskHost>,
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
    state: Mutex<AttemptState>,
    changed: Notify,
}

/// Retains attempts abandoned by runner waiters until actual service settlement.
/// An unresolved service failure stays registered for trusted reconciliation;
/// it never authorizes resubmission or a Rust agent-loop fallback.
pub(crate) struct GlobalMontyDriver {
    service: ServiceClient,
    threads: Arc<dyn SessionThreadService>,
    ports: Arc<dyn GlobalTaskPortsFactory>,
    attempts: Mutex<HashMap<MontyTaskAttempt, Arc<Attempt>>>,
    max_attempts: usize,
}

/// Private host ownership plus its actual service receipt for reconciliation.
pub(crate) type MontySettlement = (Arc<MontyTaskHost>, Arc<TaskReceipt>);

impl GlobalMontyDriver {
    pub(crate) fn new(
        service: ServiceClient,
        threads: Arc<dyn SessionThreadService>,
        ports: Arc<dyn GlobalTaskPortsFactory>,
        max_attempts: usize,
    ) -> Result<Self, AgentLoopDriverError> {
        if max_attempts == 0 {
            return Err(invalid("global Monty attempt capacity must be positive"));
        }
        Ok(Self {
            service,
            threads,
            ports,
            attempts: Mutex::new(HashMap::new()),
            max_attempts,
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
        if attempts.len() >= self.max_attempts {
            return Err(AgentLoopDriverError::Unavailable {
                reason: "Monty retained attempt capacity exhausted".into(),
            });
        }
        let entry = Arc::new(Attempt {
            host,
            state: Mutex::new(AttemptState {
                cancelled: false,
                phase: Phase::Preparing,
            }),
            changed: Notify::new(),
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

    /// Trusted supervisor receipt for a dropped turn waiter or failed attempt.
    /// The service's real outcome and late answers stay attached to the host.
    /// Removing this entry transfers reconciliation responsibility to the caller.
    pub(crate) fn take_settlement(
        &self,
        attempt: MontyTaskAttempt,
    ) -> Result<Option<MontySettlement>, AgentLoopDriverError> {
        let entry = self
            .attempts
            .lock()
            .map_err(|_| failed("monty_attempt_registry_failed"))?
            .get(&attempt)
            .cloned();
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
                    self.remove(attempt)?;
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
        self.remove(attempt)?;
        Ok(Some((entry.host.clone(), receipt)))
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
        let ports = match self.ports.build(host.clone()).await {
            Ok(ports) => ports,
            Err(error) => {
                drop(guard);
                self.remove(host.attempt())?;
                return Err(error);
            }
        };
        // Identity, user content and complete eligible history are typed values,
        // never Python source. Technical transport limits reject oversized work;
        // they must not silently turn history into an empty or truncated list.
        let input = TaskInput {
            conversation_id: context.thread_id.to_string(),
            message_id: input.message.message_id.to_string(),
            turn_id: context.turn_id.to_string(),
            run_id: context.run_id.to_string(),
            user_input: input
                .message
                .content
                .ok_or_else(|| failed("monty_admitted_input_invalid"))?,
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
        let ticket = {
            let mut state = entry
                .state
                .lock()
                .map_err(|_| failed("monty_attempt_state_failed"))?;
            if state.cancelled {
                return Err(failed("monty_task_cancelled"));
            }
            let ticket = self
                .service
                .submit(input, ports)
                .map_err(|_| failed("monty_task_admission_failed"))?;
            state.phase = Phase::Submitted(ticket.control());
            entry.changed.notify_waiters();
            ticket
        };
        let receipt = ticket
            .wait()
            .await
            .map_err(|_| failed("monty_service_reconciliation_required"))?;
        match &receipt.outcome {
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
                self.ports.settle(host.clone(), receipt.clone()).await?;
                drop(guard);
                self.remove(host.attempt())?;
                Ok(exit)
            }
            TaskOutcome::Failed { reason_kind } => {
                self.ports.settle(host.clone(), receipt.clone()).await?;
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
        tokio::time::timeout(Duration::from_secs(5), async {
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
                    control
                        .wait()
                        .await
                        .map_err(|_| failed("monty_service_reconciliation_required"))?;
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
