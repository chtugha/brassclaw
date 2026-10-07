//! Owned admission/port construction for the global Monty driver.
//!
//! Catalogue publication and boot remain separate owners. This factory never
//! matches, executes a Tool, starts a VM or supplies a fallback catalogue. Its
//! caller must supply the approved-catalogue owner before application wiring.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use brassclaw_monty_host::service::{TaskInput, TaskOutcome, TaskPorts, TaskReceipt};
use brassclaw_pg::PgPool;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_turns::{
    LoopMessageRef,
    run_profile::{AgentLoopDriverError, MontyTaskAttempt},
};
use serde_json::json;

use crate::{
    global_monty_driver::GlobalTaskPortsFactory,
    global_monty_owner::GlobalOwnerCheck,
    global_recipe_ports::{GlobalRecipePorts, MontyTaskCatalogue},
    pg_monty_admission::PgMontyAdmission,
};

/// Capture a complete coherent catalogue view for this admitted task, without
/// selection or execution. Production implementations retain approved exact
/// revisions, combination reviews and actual implementations. Draft validation
/// uses its own explicit provider; it cannot become a production approval flag.
#[async_trait]
pub(crate) trait MontyCatalogueProvider: Send + Sync {
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError>;
}

struct RetainedTask {
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    ports: Option<Arc<GlobalRecipePorts>>,
    receipt: Option<Arc<TaskReceipt>>,
    settled: bool,
}

/// Transfers the real host, private admission address, child/Tool port state
/// and service receipt together to a trusted reconciliation owner. This does
/// not authorize retry or declare any external effect reconciled.
pub(crate) type OwnedMontySettlement = (
    Arc<MontyTaskHost>,
    Arc<PgMontyAdmission>,
    Arc<GlobalRecipePorts>,
    Arc<TaskReceipt>,
);

/// Retains the exact private address before database I/O, and actual ports and
/// receipts until acknowledged settlement. Failed/abandoned preparation and
/// uncertain persistence retain evidence and consume capacity; neither outcome
/// authorizes creating a replacement admission or replaying external work.
pub(crate) struct OwnedGlobalTaskFactory {
    pool: Arc<PgPool>,
    ownership: GlobalOwnerCheck,
    catalogue: Arc<dyn MontyCatalogueProvider>,
    capacity: usize,
    tasks: Mutex<HashMap<MontyTaskAttempt, RetainedTask>>,
}

impl OwnedGlobalTaskFactory {
    pub(crate) fn new(
        pool: Arc<PgPool>,
        ownership: GlobalOwnerCheck,
        catalogue: Arc<dyn MontyCatalogueProvider>,
        capacity: usize,
    ) -> Result<Self, AgentLoopDriverError> {
        if capacity == 0 {
            return Err(failed("monty_task_capacity_invalid"));
        }
        Ok(Self {
            pool,
            ownership,
            catalogue,
            capacity,
            tasks: Mutex::new(HashMap::new()),
        })
    }

    fn tasks(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, HashMap<MontyTaskAttempt, RetainedTask>>,
        AgentLoopDriverError,
    > {
        self.tasks
            .lock()
            .map_err(|_| failed("monty_task_registry_failed"))
    }

    pub(crate) fn take_failed_settlement(
        &self,
        attempt: MontyTaskAttempt,
    ) -> Result<Option<OwnedMontySettlement>, AgentLoopDriverError> {
        let mut tasks = self.tasks()?;
        let Some(task) = tasks.get(&attempt) else {
            return Ok(None);
        };
        if !task.settled
            || !task
                .receipt
                .as_ref()
                .is_some_and(|receipt| matches!(receipt.outcome, TaskOutcome::Failed { .. }))
            || task.ports.is_none()
        {
            return Err(failed("monty_service_reconciliation_required"));
        }
        let task = tasks
            .remove(&attempt)
            .ok_or_else(|| failed("monty_task_registry_failed"))?;
        Ok(Some((
            task.host,
            task.admission,
            task.ports
                .ok_or_else(|| failed("monty_task_registry_failed"))?,
            task.receipt
                .ok_or_else(|| failed("monty_task_registry_failed"))?,
        )))
    }
}

#[async_trait]
impl GlobalTaskPortsFactory for OwnedGlobalTaskFactory {
    async fn build(
        &self,
        host: Arc<MontyTaskHost>,
        input: &TaskInput,
    ) -> Result<Arc<dyn TaskPorts>, AgentLoopDriverError> {
        let context = host.run_context();
        // The message ID is opaque typed data, not a UUID engine address. Check
        // the handoff before allocating an admission or exposing task ports.
        if input.conversation_id != context.thread_id.as_str()
            || input.turn_id != context.turn_id.to_string()
            || input.run_id != context.run_id.to_string()
            || context
                .accepted_message_ref
                .as_ref()
                .and_then(|r| r.as_str().strip_prefix("msg:"))
                != Some(input.message_id.as_str())
        {
            return Err(failed("monty_admission_identity_invalid"));
        }
        self.ownership
            .check()
            .await
            .map_err(|_| failed("monty_instance_ownership_failed"))?;
        let admission = Arc::new(PgMontyAdmission::prepare(
            self.pool.clone(),
            context,
            host.attempt(),
        )?);
        {
            let mut tasks = self.tasks()?;
            if tasks.contains_key(&host.attempt()) {
                return Err(failed("monty_admission_replay_requires_recovery"));
            }
            if tasks.len() >= self.capacity {
                return Err(AgentLoopDriverError::Unavailable {
                    reason: "Monty retained task capacity exhausted".into(),
                });
            }
            tasks.insert(
                host.attempt(),
                RetainedTask {
                    host: host.clone(),
                    admission: admission.clone(),
                    ports: None,
                    receipt: None,
                    settled: false,
                },
            );
        }
        // Retention precedes every await below, including a lost COMMIT reply
        // and a caller dropping this future. No registry lock crosses I/O.
        admission.persist_reservation().await?;
        let catalogue = self
            .catalogue
            .capture(host.clone(), admission.clone(), input)
            .await?;
        self.ownership
            .check()
            .await
            .map_err(|_| failed("monty_instance_ownership_failed"))?;
        let ports = Arc::new(GlobalRecipePorts::new(
            host.clone(),
            admission,
            catalogue,
            input.user_input.clone(),
            self.ownership.clone(),
        ));
        self.tasks()?
            .get_mut(&host.attempt())
            .ok_or_else(|| failed("monty_task_registry_failed"))?
            .ports = Some(ports.clone());
        Ok(ports)
    }

    async fn settle(
        &self,
        host: Arc<MontyTaskHost>,
        receipt: Arc<TaskReceipt>,
    ) -> Result<(), AgentLoopDriverError> {
        let outcome = match &receipt.outcome {
            TaskOutcome::Completed { reply_ref } => {
                let reference = LoopMessageRef::new(reply_ref.clone())
                    .map_err(|_| failed("monty_reply_reference_invalid"))?;
                host.published_reply_content(&reference)
                    .map_err(|_| failed("monty_reply_not_published"))?;
                if !receipt.withheld.is_empty() || host.has_withheld_results() {
                    return Err(failed("monty_service_reconciliation_required"));
                }
                json!({"status":"completed", "reply_ref":reply_ref})
            }
            TaskOutcome::Failed { reason_kind } => {
                json!({"status":"failed", "reason_kind":reason_kind})
            }
        };
        let admission = {
            let mut tasks = self.tasks()?;
            let task = tasks
                .get_mut(&host.attempt())
                .ok_or_else(|| failed("monty_task_registry_failed"))?;
            if !Arc::ptr_eq(&task.host, &host)
                || task.ports.is_none()
                || task
                    .receipt
                    .as_ref()
                    .is_some_and(|previous| !Arc::ptr_eq(previous, &receipt))
            {
                return Err(failed("monty_task_settlement_conflict"));
            }
            task.receipt = Some(receipt.clone());
            task.admission.clone()
        };
        admission.settle(outcome).await?;
        let mut tasks = self.tasks()?;
        if let Some(task) = tasks.get_mut(&host.attempt()) {
            // Another acknowledgement may have already transferred this exact
            // settlement. Never mark or release a newly registered reservation
            // just because it reuses the same run/attempt address.
            if !Arc::ptr_eq(&task.admission, &admission) {
                return Err(failed("monty_task_settlement_conflict"));
            }
            task.settled = true;
        }
        // Failed tasks retain actual child/Tool evidence for reconciliation.
        // Only a real published completion with acknowledged durable settlement
        // frees this bounded slot. The global driver still owns its receipt.
        if matches!(receipt.outcome, TaskOutcome::Completed { .. }) {
            tasks.remove(&host.attempt());
        }
        Ok(())
    }
}

fn failed(reason: &str) -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: reason.into(),
    }
}
