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
use brassclaw_monty_host::service::{
    ServiceClient, TaskInput, TaskOutcome, TaskPorts, TaskReceipt, UnsubmittedTask,
};
use brassclaw_pg::PgPool;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_turns::{
    LoopMessageRef, TurnRunId,
    run_profile::{
        AgentLoopDriverError, AgentLoopHostError, CapabilitySurfaceVersion, MontyTaskAttempt,
    },
};
use serde_json::{Value, json};

use crate::{
    global_monty_driver::{
        CancellationAckSettings, CancellationAckSource, GlobalTaskPortsFactory, PreparedGlobalTask,
    },
    global_monty_owner::GlobalOwnerCheck,
    global_recipe_ports::{GlobalRecipePorts, MontyTaskCatalogue, RecipeCapacitySource},
    monty_attempt_retention::{AttemptRetentionLease, AttemptRetentionRegistry},
    pg_monty_admission::PgMontyAdmission,
};

/// Capture a complete coherent catalogue view for this admitted task, without
/// execution. Production implementations retain approved exact
/// revisions, combination reviews and actual implementations. Draft validation
/// uses its own explicit provider; it cannot become a production approval flag.
/// Host-only child preparation from private durable provenance. Selection is
/// explicit, never inferred from a user message or a fabricated intent match.
pub(crate) struct CapturedChildRecipe {
    pub(crate) catalogue: Arc<dyn MontyTaskCatalogue>,
    pub(crate) selected: crate::global_recipe_ports::SelectedMontyRecipe,
    pub(crate) dependency_parent: Arc<dyn TaskPorts>,
}

#[async_trait]
pub(crate) trait MontyCatalogueProvider: Send + Sync {
    /// Retain only a weak scheduling dependency after the actual task ports
    /// exist. This hook neither admits work nor supplies authority.
    fn observe_task_ports(
        &self,
        _host: &Arc<MontyTaskHost>,
        _admission: &Arc<PgMontyAdmission>,
        _ports: std::sync::Weak<dyn TaskPorts>,
    ) -> Result<(), AgentLoopDriverError> {
        Ok(())
    }

    /// Completed tasks may release acknowledged child observations. Unknown
    /// admissions/effects remain with their original owner for reconciliation.
    fn release_task(&self, _attempt: MontyTaskAttempt) -> Result<(), AgentLoopDriverError> {
        Ok(())
    }
    /// Acknowledge only terminal child results already received by their
    /// original native owner. This must not admit, dispatch or resume a child,
    /// resolve an unknown admission, or turn the original failed Tool into a
    /// successful return value.
    async fn reconcile_received_child_results(
        &self,
        _attempt: MontyTaskAttempt,
    ) -> Result<(), AgentLoopDriverError> {
        Ok(())
    }
    /// A provider without the complete child admission/retention contract must
    /// leave ordinary entry unchanged. Returning a source requires verified
    /// parent attempt/admission, child identity and the parent's pinned view.
    async fn capture_child(
        &self,
        _host: Arc<MontyTaskHost>,
        _admission: Arc<PgMontyAdmission>,
        _input: &TaskInput,
    ) -> Result<Option<CapturedChildRecipe>, AgentLoopDriverError> {
        Ok(None)
    }

    /// Resolve host-owned durable internal work before ordinary chat matching.
    /// The default preserves normal catalogues and grants no internal entry.
    async fn capture_internal(
        &self,
        _host: Arc<MontyTaskHost>,
        _admission: Arc<PgMontyAdmission>,
        _input: &TaskInput,
    ) -> Result<
        Option<Arc<dyn crate::global_recipe_ports::InternalRecipeSource>>,
        AgentLoopDriverError,
    > {
        Ok(None)
    }
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError>;
}

/// Retain an already selected request-local provider exchange. This lifecycle
/// boundary neither advertises commands nor executes a Tool or model request.
/// The owning adapter must keep the exact port alive through cancellation and
/// settlement, and make repeated disconnects safe for that same exchange.
#[async_trait]
pub(crate) trait TaskProviderLease: Send + Sync {
    fn surface_version(&self) -> &CapabilitySurfaceVersion;
    fn is_live(&self) -> bool;
    async fn finish(&self);
}

pub(crate) trait TaskProviderBinding: Send + Sync {
    fn retain(
        self: Arc<Self>,
        run_id: TurnRunId,
    ) -> Result<Option<Arc<dyn TaskProviderLease>>, AgentLoopHostError>;
}

struct RetainedTask {
    host: Arc<MontyTaskHost>,
    retention: Arc<AttemptRetentionLease>,
    admission: Arc<PgMontyAdmission>,
    ports: Option<Arc<GlobalRecipePorts>>,
    receipt: Option<Arc<TaskReceipt>>,
    execution_report: Option<Value>,
    settled: bool,
    provider_watch: Option<tokio_util::task::AbortOnDropHandle<()>>,
    provider_port: Option<Arc<dyn TaskProviderLease>>,
    unsubmitted: Option<Arc<UnsubmittedTask>>,
}

/// Transfers the real host, private admission address, child/Tool port state
/// and service receipt together to a trusted reconciliation owner. This does
/// not authorize retry or declare any external effect reconciled.
#[cfg(test)]
pub(crate) type OwnedMontySettlement = (
    Arc<MontyTaskHost>,
    Arc<PgMontyAdmission>,
    Arc<GlobalRecipePorts>,
    Arc<TaskReceipt>,
    Arc<AttemptRetentionLease>,
);

/// Retains the exact private address before database I/O, and actual ports and
/// receipts until acknowledged settlement. Failed/abandoned preparation and
/// uncertain persistence retain evidence and consume capacity; neither outcome
/// authorizes creating a replacement admission or replaying external work.
pub(crate) struct OwnedGlobalTaskFactory {
    root: brassclaw_monty_host::global::RootExecutionIdentity,
    pool: Arc<PgPool>,
    ownership: GlobalOwnerCheck,
    catalogue: Arc<dyn MontyCatalogueProvider>,
    recipe_capacity: Arc<dyn RecipeCapacitySource>,
    cancellation: Arc<dyn CancellationAckSource>,
    retention: Arc<AttemptRetentionRegistry>,
    tasks: Mutex<HashMap<MontyTaskAttempt, RetainedTask>>,
    mcp_provider_binding: Option<Arc<dyn TaskProviderBinding>>,
}

impl OwnedGlobalTaskFactory {
    pub(crate) fn new(
        pool: Arc<PgPool>,
        ownership: GlobalOwnerCheck,
        catalogue: Arc<dyn MontyCatalogueProvider>,
        service: ServiceClient,
        recipe_capacity: Arc<dyn RecipeCapacitySource>,
        cancellation: Arc<dyn CancellationAckSource>,
        mcp_provider_binding: Option<Arc<dyn TaskProviderBinding>>,
    ) -> Result<Self, AgentLoopDriverError> {
        if service.retained_attempts().limit == 0 {
            return Err(failed("monty_retention_unavailable"));
        }
        Ok(Self {
            root: service.root_identity(),
            pool,
            ownership,
            catalogue,
            recipe_capacity,
            cancellation,
            retention: AttemptRetentionRegistry::new(service),
            tasks: Mutex::new(HashMap::new()),
            mcp_provider_binding,
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

    async fn dependency_parent(
        &self,
        host: &MontyTaskHost,
        input: &TaskInput,
    ) -> Result<Option<Arc<dyn TaskPorts>>, AgentLoopDriverError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("mcp_parent_database_failed"))?;
        let row = client.query_opt("SELECT c.parent_run_id,c.owner_scope,c.command,c.phase,
            e.parent_run_id,e.owner_scope,e.active,e.disconnected_at IS NULL,e.parent_surface_version
            FROM brassclaw_mcp_chat_calls c JOIN brassclaw_mcp_exchanges e USING(exchange_id)
            WHERE c.call_id=$1", &[&host.attempt().run_id.as_uuid()])
            .await.map_err(|_| failed("mcp_parent_database_failed"))?;
        drop(client);
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(version) = row.get::<_, Option<String>>(8) else {
            // Historical/protocol-only exchanges cannot identify a parent
            // attempt. They receive no dependent slot and infer no successor.
            return Ok(None);
        };
        let parent_run: uuid::Uuid = row.get(0);
        let owner: brassclaw_threads::ThreadScope = serde_json::from_value(row.get(1))
            .map_err(|_| failed("mcp_parent_identity_invalid"))?;
        let context = host.run_context();
        if parent_run == host.attempt().run_id.as_uuid()
            || row.get::<_, uuid::Uuid>(4) != parent_run
            || row.get::<_, Value>(1) != row.get::<_, Value>(5)
            || row.get::<_, String>(2) != input.user_input
            || row.get::<_, i16>(3) != 1
            || !row.get::<_, bool>(6)
            || !row.get::<_, bool>(7)
            || context.scope.tenant_id != owner.tenant_id
            || context.scope.agent_id.as_ref() != Some(&owner.agent_id)
            || context.scope.project_id != owner.project_id
            || context.scope.explicit_owner_user_id() != owner.owner_user_id.as_ref()
        {
            return Err(failed("mcp_parent_identity_invalid"));
        }
        let (parent_host, admission, ports, provider) = {
            let tasks = self.tasks()?;
            let mut matching = tasks.values().filter(|task| {
                task.host.attempt().run_id.as_uuid() == parent_run
                    && task.provider_port.as_ref().is_some_and(|port| {
                        port.surface_version().as_str() == version && port.is_live()
                    })
            });
            let parent = matching
                .next()
                .ok_or_else(|| failed("mcp_parent_attempt_fenced"))?;
            if matching.next().is_some()
                || parent.settled
                || parent.receipt.is_some()
                || parent.unsubmitted.is_some()
            {
                return Err(failed("mcp_parent_attempt_fenced"));
            }
            let parent_context = parent.host.run_context();
            if parent_context.scope.tenant_id != context.scope.tenant_id
                || parent_context.scope.agent_id != context.scope.agent_id
                || parent_context.scope.project_id != context.scope.project_id
                || parent_context.scope.thread_owner != context.scope.thread_owner
            {
                return Err(failed("mcp_parent_identity_invalid"));
            }
            (
                parent.host.clone(),
                parent.admission.clone(),
                parent
                    .ports
                    .clone()
                    .ok_or_else(|| failed("mcp_parent_attempt_fenced"))?,
                parent
                    .provider_port
                    .clone()
                    .ok_or_else(|| failed("mcp_parent_attempt_fenced"))?,
            )
        };
        parent_host
            .check_dispatch_open()
            .map_err(|_| failed("mcp_parent_attempt_fenced"))?;
        admission.check_live_dependency().await?;
        parent_host
            .check_dispatch_open()
            .map_err(|_| failed("mcp_parent_attempt_fenced"))?;
        if !provider.is_live() {
            return Err(failed("mcp_parent_attempt_fenced"));
        }
        Ok(Some(ports))
    }

    #[cfg(test)]
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
            task.retention,
        )))
    }
}

#[async_trait]
impl GlobalTaskPortsFactory for OwnedGlobalTaskFactory {
    fn retention_registry(&self) -> Arc<AttemptRetentionRegistry> {
        self.retention.clone()
    }
    fn cancellation_acknowledgement(
        &self,
    ) -> Result<CancellationAckSettings, AgentLoopDriverError> {
        self.cancellation.current()
    }
    async fn build(
        &self,
        host: Arc<MontyTaskHost>,
        mut input: TaskInput,
    ) -> Result<PreparedGlobalTask, AgentLoopDriverError> {
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
        let provider_port = if context.trusted_internal_turn {
            None
        } else if let Some(binding) = &self.mcp_provider_binding {
            if let Some(port) = binding
                .clone()
                .retain(context.run_id)
                .map_err(|_| failed("mcp_exchange_unavailable"))?
            {
                let surface = host
                    .visible_capabilities()
                    .await
                    .map_err(|_| failed("mcp_exchange_unavailable"))?;
                if &surface.version != port.surface_version() {
                    return Err(failed("mcp_exchange_task_selection_conflict"));
                }
                Some(port)
            } else {
                None
            }
        } else {
            None
        };
        let retention = self.retention.retain(&host)?;
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
            let provider_watch = provider_port.as_ref().map(|port| {
                let host = host.clone();
                let port = port.clone();
                tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                    host.cancellation_requested().await;
                    port.finish().await;
                }))
            });
            tasks.insert(
                host.attempt(),
                RetainedTask {
                    host: host.clone(),
                    retention,
                    admission: admission.clone(),
                    ports: None,
                    receipt: None,
                    execution_report: None,
                    settled: false,
                    provider_watch,
                    provider_port,
                    unsubmitted: None,
                },
            );
        }
        // Retention precedes every await below, including a lost COMMIT reply
        // and a caller dropping this future. No registry lock crosses I/O.
        admission.persist_reservation().await?;
        let observation_admission = admission.clone();
        let child = self
            .catalogue
            .capture_child(host.clone(), admission.clone(), &input)
            .await?;
        let dependency_parent;
        let ports = if let Some(child) = child {
            if context.trusted_internal_turn || child.catalogue.catalogue_reference().is_none() {
                return Err(failed("child_recipe_admission_invalid"));
            }
            input.entry = brassclaw_monty_host::service::TaskEntry::ChildRecipe;
            dependency_parent = Some(child.dependency_parent);
            GlobalRecipePorts::new(
                host.clone(),
                admission,
                child.catalogue,
                input.user_input.clone(),
                self.ownership.clone(),
                self.recipe_capacity.clone(),
            )
            .with_child_recipe(child.selected)
            .map_err(|_| failed("child_recipe_admission_invalid"))?
        } else {
            dependency_parent = self.dependency_parent(&host, &input).await?;
            let internal = self
                .catalogue
                .capture_internal(host.clone(), admission.clone(), &input)
                .await?;
            if let Some(source) = internal {
                if !context.trusted_internal_turn || source.reference() != input.user_input {
                    return Err(failed("internal_recipe_admission_invalid"));
                }
                input.entry = brassclaw_monty_host::service::TaskEntry::InternalRecipe {
                    name: source.name().to_owned(),
                };
                GlobalRecipePorts::for_internal(
                    host.clone(),
                    admission,
                    source,
                    Some(self.ownership.clone()),
                    false,
                    self.recipe_capacity.clone(),
                )
            } else {
                let catalogue = self
                    .catalogue
                    .capture(host.clone(), admission.clone(), &input)
                    .await?;
                GlobalRecipePorts::new(
                    host.clone(),
                    admission,
                    catalogue,
                    input.user_input.clone(),
                    self.ownership.clone(),
                    self.recipe_capacity.clone(),
                )
            }
        };
        self.ownership
            .check()
            .await
            .map_err(|_| failed("monty_instance_ownership_failed"))?;
        let ports = Arc::new(ports.with_dependency_parent(dependency_parent));
        let scheduled: Arc<dyn TaskPorts> = ports.clone();
        self.catalogue.observe_task_ports(
            &host,
            &observation_admission,
            Arc::downgrade(&scheduled),
        )?;
        self.tasks()?
            .get_mut(&host.attempt())
            .ok_or_else(|| failed("monty_task_registry_failed"))?
            .ports = Some(ports.clone());
        Ok(PreparedGlobalTask { input, ports })
    }

    async fn settle_unsubmitted(
        &self,
        host: Arc<MontyTaskHost>,
        proof: Arc<UnsubmittedTask>,
    ) -> Result<(), AgentLoopDriverError> {
        let (admission, provider) = {
            let mut tasks = self.tasks()?;
            let task = tasks
                .get_mut(&host.attempt())
                .ok_or_else(|| failed("monty_task_registry_failed"))?;
            let ports: Arc<dyn TaskPorts> = task
                .ports
                .clone()
                .ok_or_else(|| failed("monty_task_registry_failed"))?;
            if !Arc::ptr_eq(&host, &task.host)
                || !proof.matches(self.root, &host.attempt().run_id.to_string(), &ports)
                || task.receipt.is_some()
                || task.execution_report.is_some()
                || task.settled
                || task
                    .unsubmitted
                    .as_ref()
                    .is_some_and(|old| !Arc::ptr_eq(old, &proof))
                || host.review_dispatch_counts() != (0, 0)
            {
                return Err(failed("monty_unsubmitted_identity_conflict"));
            }
            ports.fence();
            // Retain before any I/O, including an unknown COMMIT reply.
            task.unsubmitted = Some(proof.clone());
            (task.admission.clone(), task.provider_port.clone())
        };
        if let Some(provider) = provider {
            provider.finish().await;
        }
        admission.settle_unsubmitted(proof.reason()).await?;
        let mut tasks = self.tasks()?;
        let task = tasks
            .get(&host.attempt())
            .ok_or_else(|| failed("monty_task_registry_failed"))?;
        if !Arc::ptr_eq(&task.admission, &admission) {
            return Err(failed("monty_task_settlement_conflict"));
        }
        task.retention.unsubmitted_settled()?;
        self.catalogue.release_task(host.attempt())?;
        tasks.remove(&host.attempt());
        Ok(())
    }

    async fn settle(
        &self,
        host: Arc<MontyTaskHost>,
        receipt: Arc<TaskReceipt>,
    ) -> Result<(), AgentLoopDriverError> {
        let mut outcome = match &receipt.outcome {
            TaskOutcome::InternalCompleted { receipt_ref } => {
                if !host.run_context().trusted_internal_turn
                    || host.finalized_reply_ref().is_some()
                    || !receipt.withheld.is_empty()
                    || host.has_withheld_results()
                {
                    return Err(failed("monty_internal_result_invalid"));
                }
                let reference = crate::monty_result_reference::result_reference(
                    host.attempt().run_id,
                    receipt_ref,
                )
                .ok_or_else(|| failed("monty_internal_result_invalid"))?;
                json!({"status":"completed","result_ref":reference.as_str(),"internal_receipt":receipt_ref})
            }
            TaskOutcome::Completed { reply_ref } => {
                let reference = LoopMessageRef::new(reply_ref.clone())
                    .map_err(|_| failed("monty_reply_reference_invalid"))?;
                // Settlement records an already-issued service outcome. A
                // dropped waiter may have fenced active host calls after the
                // real reply and root completion; that fence must not erase
                // their evidence or prevent the original audit write.
                if host.finalized_reply_ref().as_ref() != Some(&reference) {
                    return Err(failed("monty_reply_not_published"));
                }
                if !receipt.withheld.is_empty() || host.has_withheld_results() {
                    return Err(failed("monty_service_reconciliation_required"));
                }
                json!({"status":"completed", "reply_ref":reply_ref})
            }
            TaskOutcome::Failed { reason_kind } => {
                json!({"status":"failed", "reason_kind":reason_kind})
            }
        };
        let (admission, ports, retained_report, provider_port) = {
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
            (
                task.admission.clone(),
                task.ports
                    .clone()
                    .ok_or_else(|| failed("monty_task_registry_failed"))?,
                task.execution_report.clone(),
                task.provider_port.clone(),
            )
        };
        // A transient journal failure must not discard a received Tool result.
        // This retries only its same idempotent evidence acknowledgement; the
        // original failed VM outcome stays failed and execution never resumes.
        ports
            .reconcile_received_native_results()
            .await
            .map_err(|_| failed("monty_native_result_reconciliation_required"))?;
        self.catalogue
            .reconcile_received_child_results(host.attempt())
            .await?;
        let report = match retained_report {
            Some(report) => report,
            None => {
                let report = ports
                    .settlement_report(&receipt)
                    .await
                    .map_err(|_| failed("monty_task_execution_evidence_invalid"))?;
                let mut tasks = self.tasks()?;
                let task = tasks
                    .get_mut(&host.attempt())
                    .ok_or_else(|| failed("monty_task_registry_failed"))?;
                if !Arc::ptr_eq(&task.admission, &admission)
                    || task
                        .execution_report
                        .as_ref()
                        .is_some_and(|previous| previous != &report)
                {
                    return Err(failed("monty_task_settlement_conflict"));
                }
                task.execution_report = Some(report.clone());
                report
            }
        };
        if let TaskOutcome::InternalCompleted { receipt_ref } = &receipt.outcome
            && (report["format"] != "monty-internal-task-execution/1"
                || report["root_completed"] != true
                || report["all_selected_recipes_complete"] != true
                || report["internal_receipt"] != receipt_ref.as_str()
                || !report["reply"].is_null())
        {
            return Err(failed("monty_internal_result_invalid"));
        }
        // Keep the original report across unknown commit acknowledgement. The
        // same transaction records outcome and child completion together; audit
        // recovery never rereads latest, restores dispatch or repeats an effect.
        outcome["execution"] = report;
        if let Some(port) = &provider_port {
            port.finish().await;
        }
        admission.settle(outcome).await?;
        // Optional discovery observes only acknowledged durable evidence. Its
        // pending/error state cannot replay or invalidate a completed task.
        ports.refresh_command_qualification().await;
        let mut tasks = self.tasks()?;
        if let Some(task) = tasks.get_mut(&host.attempt()) {
            // Another acknowledgement may have already transferred this exact
            // settlement. Never mark or release a newly registered reservation
            // just because it reuses the same run/attempt address.
            if !Arc::ptr_eq(&task.admission, &admission) {
                return Err(failed("monty_task_settlement_conflict"));
            }
            task.settled = true;
            task.provider_watch.take();
        }
        // Failed tasks retain actual child/Tool evidence for reconciliation.
        // Only a verified reply/result with acknowledged durable settlement
        // frees this bounded slot. The global driver still owns its receipt.
        if matches!(
            receipt.outcome,
            TaskOutcome::Completed { .. } | TaskOutcome::InternalCompleted { .. }
        ) {
            self.catalogue.release_task(host.attempt())?;
            if let Some(task) = tasks.get(&host.attempt()) {
                task.retention.completed()?;
            }
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
