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

pub(crate) struct CapturedPrivateRecipe {
    pub(crate) definition: Arc<crate::private_work_definition::PrivateWorkDefinition>,
    pub(crate) source: Arc<dyn crate::global_recipe_ports::InternalRecipeSource>,
}

/// Capture a complete coherent catalogue view for this admitted task, without
/// execution. Production implementations retain approved exact
/// revisions, combination reviews and actual implementations. Draft validation
/// uses its own explicit provider; it cannot become a production approval flag.
/// Host-only child preparation from private durable provenance. Selection is
/// explicit, never inferred from a user message or a fabricated intent match.
pub(crate) struct BoundChildRecipe {
    pub(crate) catalogue: Arc<dyn MontyTaskCatalogue>,
    pub(crate) selected: crate::global_recipe_ports::SelectedMontyRecipe,
    pub(crate) dependency_parent: Arc<dyn TaskPorts>,
}

/// Captured native lineage and view only. No consumer runtime exists yet.
#[async_trait]
pub(crate) trait CapturedChildBindings: Send + Sync {
    fn captured_programs(
        &self,
    ) -> Vec<Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>>;
    async fn bind(&self) -> Result<BoundChildRecipe, AgentLoopDriverError>;
}
pub(crate) struct CapturedChildRecipe {
    pub(crate) bindings: Arc<dyn CapturedChildBindings>,
    pub(crate) parent_model_selection:
        Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>>,
}

/// The catalogue owner's one immutable selection, before task-local runtime
/// construction. Binding consumes that captured selection; it never matches
/// again, reads latest or creates another admission.
#[async_trait]
pub(crate) trait CapturedMontyCatalogue: Send + Sync {
    fn selected_workflow(
        &self,
    ) -> Result<
        Option<Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>>,
        AgentLoopDriverError,
    >;
    fn captured_programs(
        &self,
    ) -> Vec<Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>>;
    async fn bind(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError>;
}

#[async_trait]
pub(crate) trait MontyCatalogueProvider: Send + Sync {
    #[cfg(test)]
    async fn prepare_private_validator(
        &self,
        _submission_id: uuid::Uuid,
    ) -> Result<brassclaw_turns::private_work::PrivateWorkState, crate::RebornBuildError> {
        Err(crate::RebornBuildError::InvalidConfig {
            reason: "private validator preparation unavailable".into(),
        })
    }

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
    async fn capture_private(
        &self,
        _host: Arc<MontyTaskHost>,
        _admission: Arc<PgMontyAdmission>,
    ) -> Result<CapturedPrivateRecipe, AgentLoopDriverError> {
        Err(failed("private_recipe_source_unavailable"))
    }

    async fn prepare(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Arc<dyn CapturedMontyCatalogue>, AgentLoopDriverError>;
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
    preparation: Option<Arc<CapturedTaskPreparation>>,
    ports: Option<Arc<GlobalRecipePorts>>,
    receipt: Option<Arc<TaskReceipt>>,
    execution_report: Option<Value>,
    settled: bool,
    provider_watch: Option<tokio_util::task::AbortOnDropHandle<()>>,
    provider_port: Option<Arc<dyn TaskProviderLease>>,
    unsubmitted: Option<Arc<UnsubmittedTask>>,
}

/// Retained before model/configuration awaits, including caller cancellation.
/// This owns only captured data/binding constructors, never another work queue.
enum CapturedTaskPreparation {
    Child(CapturedChildRecipe),
    Internal(Arc<dyn crate::global_recipe_ports::InternalRecipeSource>),
    Ordinary(Arc<dyn CapturedMontyCatalogue>),
}
impl CapturedTaskPreparation {
    fn captured_programs(
        &self,
    ) -> Result<
        Vec<Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>>,
        AgentLoopDriverError,
    > {
        match self {
            Self::Child(child) => Ok(child.bindings.captured_programs()),
            Self::Ordinary(captured) => Ok(captured.captured_programs()),
            Self::Internal(source) => source
                .captured_program()
                .map(|program| vec![program])
                .ok_or_else(|| failed("internal_recipe_capture_missing")),
        }
    }
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
    #[cfg(test)]
    private_settlement_pauses: Mutex<HashMap<TurnRunId, Arc<PrivateSettlementPause>>>,
}

#[cfg(test)]
pub(crate) struct PrivateSettlementPause {
    pub(crate) reached: tokio::sync::Notify,
    pub(crate) release: tokio::sync::Notify,
}

impl OwnedGlobalTaskFactory {
    /// Hold the real settlement after its actual terminal COMMIT and before
    /// Factory cleanup/return. This does not fabricate a receipt or an ACK.
    #[cfg(test)]
    pub(crate) fn pause_private_settlement(&self, run: TurnRunId) -> Arc<PrivateSettlementPause> {
        let pause = Arc::new(PrivateSettlementPause {
            reached: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        assert!(
            self.private_settlement_pauses
                .lock()
                .unwrap()
                .insert(run, pause.clone())
                .is_none()
        );
        pause
    }
    /// Read-only retained fixture facts after an observed terminal turn. Report
    /// construction reads the actual retained ports; it never acknowledges,
    /// retries or transfers a settlement and never exposes raw task payloads.
    #[cfg(test)]
    pub(crate) async fn test_settlement_observations(
        &self,
        run: TurnRunId,
    ) -> Result<Vec<Value>, AgentLoopDriverError> {
        let selected = {
            let tasks = self.tasks()?;
            tasks
                .iter()
                .filter(|(attempt, _)| attempt.run_id == run)
                .map(|(attempt, task)| {
                    (
                        *attempt,
                        task.settled,
                        task.receipt.clone(),
                        task.ports.clone(),
                        task.execution_report.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        fn summarize(mut report: Value) -> Value {
            if let Some(fields) = report.as_object_mut() {
                let present = fields
                    .remove("internal_receipt")
                    .is_some_and(|receipt| !receipt.is_null());
                fields.insert("internal_receipt_present".into(), json!(present));
            }
            report
        }
        let mut observations = Vec::with_capacity(selected.len());
        for (attempt, settled, receipt, ports, retained_report) in selected {
            let observed_report = match (&ports, &receipt) {
                (Some(ports), Some(receipt)) => Some(
                    ports
                        .settlement_report(receipt)
                        .await
                        .map(summarize)
                        .map_err(|error| format!("{error:?}")),
                ),
                _ => None,
            };
            observations.push(json!({
                "attempt": format!("{attempt:?}"),
                "settled": settled,
                "receipt_outcome": receipt.as_ref().map(|receipt| match &receipt.outcome {
                    TaskOutcome::InternalCompleted { .. } => "internal_completed",
                    TaskOutcome::Completed { .. } => "completed",
                    TaskOutcome::Failed { reason_kind } => reason_kind.as_str(),
                }),
                "retained_report": retained_report.map(summarize),
                "observed_report": observed_report,
            }));
        }
        Ok(observations)
    }

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
            #[cfg(test)]
            private_settlement_pauses: Mutex::new(HashMap::new()),
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
        let context = host
            .run_context()
            .ok_or_else(|| failed("monty_chat_origin_required"))?;
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
            let parent_context = parent
                .host
                .run_context()
                .ok_or_else(|| failed("mcp_parent_identity_invalid"))?;
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

    #[cfg(all(test, feature = "root-llm-provider"))]
    /// Test-only view of the actual host retained by `build` before admission.
    /// Mirrors native Recipe dispatch custody; it creates no task or authority.
    pub(crate) fn retained_host_for_test(
        &self,
        thread: &brassclaw_host_api::ThreadId,
    ) -> Result<Option<Arc<MontyTaskHost>>, AgentLoopDriverError> {
        let tasks = self.tasks()?;
        let mut matching = tasks.values().filter(|task| {
            !task.settled
                && task
                    .host
                    .run_context()
                    .is_some_and(|context| &context.thread_id == thread)
        });
        let host = matching.next().map(|task| task.host.clone());
        if matching.next().is_some() {
            return Err(failed("test_task_identity_ambiguous"));
        }
        Ok(host)
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
    #[cfg(test)]
    async fn prepare_private_validator(
        &self,
        submission_id: uuid::Uuid,
    ) -> Result<brassclaw_turns::private_work::PrivateWorkState, crate::RebornBuildError> {
        self.catalogue
            .prepare_private_validator(submission_id)
            .await
    }

    fn private_attempt_retained(
        &self,
        run: brassclaw_turns::TurnRunId,
    ) -> Result<bool, AgentLoopDriverError> {
        Ok(self.tasks()?.iter().any(|(attempt, task)| {
            attempt.run_id == run && task.host.private_claim().is_some() && !task.settled
        }))
    }

    fn retention_registry(&self) -> Arc<AttemptRetentionRegistry> {
        self.retention.clone()
    }
    fn cancellation_acknowledgement(
        &self,
    ) -> Result<CancellationAckSettings, AgentLoopDriverError> {
        self.cancellation.current()
    }
    fn private_host(
        &self,
        claimed: &brassclaw_turns::private_work::ClaimedPrivateWork,
    ) -> Result<Arc<MontyTaskHost>, AgentLoopDriverError> {
        let tasks = self.tasks()?;
        let task = tasks
            .get(&claimed.attempt())
            .ok_or_else(|| failed("private_original_host_unavailable"))?;
        let original = task
            .host
            .private_claim()
            .ok_or_else(|| failed("private_original_host_invalid"))?;
        if original.request() != claimed.request()
            || original.definition_checksum() != claimed.definition_checksum()
            || original.request_checksum() != claimed.request_checksum()
            || task.settled
        {
            return Err(failed("private_original_host_invalid"));
        }
        Ok(task.host.clone())
    }

    async fn before_submit(&self, host: &Arc<MontyTaskHost>) -> Result<(), AgentLoopDriverError> {
        if host.private_claim().is_none() {
            return Ok(());
        }
        host.check_dispatch_open()
            .map_err(|_| failed("monty_admission_fenced"))?;
        let admission = {
            let tasks = self.tasks()?;
            let task = tasks
                .get(&host.attempt())
                .ok_or_else(|| failed("private_original_host_unavailable"))?;
            if !Arc::ptr_eq(&task.host, host) || task.ports.is_none() || task.settled {
                return Err(failed("private_original_host_invalid"));
            }
            task.admission.clone()
        };
        admission.mark_private_submission_intent().await
    }

    async fn build_private(
        &self,
        host: Arc<MontyTaskHost>,
    ) -> Result<PreparedGlobalTask, AgentLoopDriverError> {
        let claim = host
            .private_claim()
            .ok_or_else(|| failed("private_original_host_invalid"))?;
        let admission = {
            let tasks = self.tasks()?;
            let task = tasks
                .get(&host.attempt())
                .ok_or_else(|| failed("private_original_host_unavailable"))?;
            if !Arc::ptr_eq(&task.host, &host)
                || task.preparation.is_some()
                || task.ports.is_some()
                || task.settled
            {
                return Err(failed("private_original_host_invalid"));
            }
            task.admission.clone()
        };
        self.ownership
            .check()
            .await
            .map_err(|_| failed("monty_instance_ownership_failed"))?;
        admission.persist_reservation().await?;
        let captured = self
            .catalogue
            .capture_private(host.clone(), admission.clone())
            .await?;
        if captured.definition.checksum() != claim.definition_checksum()
            || captured.definition.root().source_checksum() != self.root.source_checksum()
        {
            return Err(failed("private_preparation_root_invalid"));
        }
        let program = captured
            .source
            .captured_program()
            .ok_or_else(|| failed("private_preparation_program_missing"))?;
        {
            let mut tasks = self.tasks()?;
            let task = tasks
                .get_mut(&host.attempt())
                .ok_or_else(|| failed("private_original_host_unavailable"))?;
            if task.preparation.is_some() {
                return Err(failed("monty_task_preparation_conflict"));
            }
            task.preparation = Some(Arc::new(CapturedTaskPreparation::Internal(
                captured.source.clone(),
            )));
        }
        // First-slice private profiles are model-free, still using the common
        // captured-source/native-binding path and current kernel dispatch guard.
        crate::task_model_capture::capture_task_models(
            &host,
            &admission,
            Some(program.as_ref()),
            None,
        )
        .await?;
        captured
            .source
            .bind_native_runtimes()
            .map_err(|_| failed("private_recipe_binding_failed"))?;
        let ports = Arc::new(
            GlobalRecipePorts::for_internal(
                host.clone(),
                admission.clone(),
                captured.source,
                Some(self.ownership.clone()),
                false,
                self.recipe_capacity.clone(),
            )
            .with_private_definition(captured.definition.clone())
            .map_err(|_| failed("private_recipe_origin_invalid"))?,
        );
        let scheduled: Arc<dyn TaskPorts> = ports.clone();
        self.catalogue
            .observe_task_ports(&host, &admission, Arc::downgrade(&scheduled))?;
        self.tasks()?
            .get_mut(&host.attempt())
            .ok_or_else(|| failed("private_original_host_unavailable"))?
            .ports = Some(ports.clone());
        self.ownership
            .check()
            .await
            .map_err(|_| failed("monty_instance_ownership_failed"))?;
        Ok(PreparedGlobalTask {
            input: captured.definition.task_input(claim).into(),
            ports,
        })
    }

    async fn build(
        &self,
        host: Arc<MontyTaskHost>,
        mut input: TaskInput,
    ) -> Result<PreparedGlobalTask, AgentLoopDriverError> {
        let context = host
            .run_context()
            .ok_or_else(|| failed("monty_chat_origin_required"))?;
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
            host.claimed_run()
                .map(|claimed| &claimed.state.source_binding_ref),
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
                    preparation: None,
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
        let preparation = if let Some(child) = self
            .catalogue
            .capture_child(host.clone(), admission.clone(), &input)
            .await?
        {
            CapturedTaskPreparation::Child(child)
        } else if let Some(source) = self
            .catalogue
            .capture_internal(host.clone(), admission.clone(), &input)
            .await?
        {
            CapturedTaskPreparation::Internal(source)
        } else {
            if host.is_result_only_completion() {
                return Err(failed("internal_recipe_source_unavailable"));
            }
            CapturedTaskPreparation::Ordinary(
                self.catalogue
                    .prepare(host.clone(), admission.clone(), &input)
                    .await?,
            )
        };
        // Keep the original selection even if a model/configuration await or
        // the caller fails. Recovery never selects a newer view in its place.
        {
            let mut tasks = self.tasks()?;
            let task = tasks
                .get_mut(&host.attempt())
                .ok_or_else(|| failed("monty_task_registry_failed"))?;
            if task.preparation.is_some() {
                return Err(failed("monty_task_preparation_conflict"));
            }
            task.preparation = Some(Arc::new(preparation));
        }
        let preparation = self
            .tasks()?
            .get(&host.attempt())
            .and_then(|task| task.preparation.clone())
            .ok_or_else(|| failed("monty_task_preparation_missing"))?;
        // The captured source must expose its actual inspected definitions
        // before model/source capture, never an empty placeholder catalogue.
        if preparation.captured_programs()?.is_empty() {
            return Err(failed("monty_task_preparation_empty"));
        }
        let parent_model_selection = match preparation.as_ref() {
            CapturedTaskPreparation::Child(child) => {
                Some(child.parent_model_selection.clone().unwrap_or_else(|| {
                    Arc::new(brassclaw_turns::run_profile::InheritedUnavailableModelSelection)
                }))
            }
            _ => None,
        };
        // Only the actual retained root workflow may declare capture without
        // accepted prefix bindings. Children keep the original family, never
        // recapture or refill it. This declaration is not dispatch qualification.
        let root_workflow = match preparation.as_ref() {
            CapturedTaskPreparation::Ordinary(captured) => captured.selected_workflow()?,
            CapturedTaskPreparation::Internal(source) => source.captured_program(),
            CapturedTaskPreparation::Child(_) => None,
        };
        crate::task_model_capture::capture_task_models(
            &host,
            &admission,
            root_workflow.as_deref(),
            parent_model_selection,
        )
        .await?;
        let dependency_parent;
        let ports = match preparation.as_ref() {
            CapturedTaskPreparation::Child(child) => {
                let child = child.bindings.bind().await?;
                if context.trusted_internal_turn || child.catalogue.catalogue_reference().is_none()
                {
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
            }
            CapturedTaskPreparation::Internal(source) => {
                dependency_parent = self.dependency_parent(&host, &input).await?;
                if source.reference() != input.user_input {
                    return Err(failed("internal_recipe_admission_invalid"));
                }
                source
                    .bind_native_runtimes()
                    .map_err(|_| failed("internal_recipe_binding_failed"))?;
                input.entry = brassclaw_monty_host::service::TaskEntry::InternalRecipe {
                    name: source.name().to_owned(),
                };
                GlobalRecipePorts::for_internal(
                    host.clone(),
                    admission,
                    source.clone(),
                    Some(self.ownership.clone()),
                    false,
                    self.recipe_capacity.clone(),
                )
            }
            CapturedTaskPreparation::Ordinary(captured) => {
                dependency_parent = self.dependency_parent(&host, &input).await?;
                let catalogue = captured.bind(host.clone(), admission.clone()).await?;
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
        Ok(PreparedGlobalTask {
            input: input.into(),
            ports,
        })
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
        if host.private_claim().is_some() {
            admission.settle_private_unsubmitted(&proof).await?;
        } else {
            admission.settle_unsubmitted(proof.reason()).await?;
        }
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
                if !host.is_result_only_completion()
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
        if host.private_claim().is_some() {
            if receipt.root != self.root
                || !receipt.withheld.is_empty()
                || host.has_withheld_results()
            {
                return Err(failed("monty_service_reconciliation_required"));
            }
            admission.settle_private_native(outcome, &receipt).await?;
            #[cfg(test)]
            {
                let pause = self
                    .private_settlement_pauses
                    .lock()
                    .unwrap()
                    .remove(&host.attempt().run_id);
                if let Some(pause) = pause {
                    pause.reached.notify_one();
                    pause.release.notified().await;
                }
            }
        } else {
            admission.settle(outcome).await?;
        }
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

impl brassclaw_turns::pg_turn_state_store::PgPrivateClaimParticipant for OwnedGlobalTaskFactory {
    fn retain_reservation(
        &self,
        claimed: &brassclaw_turns::private_work::ClaimedPrivateWork,
    ) -> Result<
        Arc<dyn brassclaw_turns::pg_turn_state_store::PgPrivateClaimReservation>,
        brassclaw_turns::TurnError,
    > {
        let invalid = || brassclaw_turns::TurnError::Unavailable {
            reason: "private reservation requires original native custody".into(),
        };
        let mut tasks = self.tasks().map_err(|_| invalid())?;
        if let Some((_, existing)) = tasks
            .iter()
            .find(|(attempt, _)| attempt.run_id == claimed.attempt().run_id)
        {
            let original = existing.host.private_claim().ok_or_else(invalid)?;
            if original.request() != claimed.request()
                || original.definition_checksum() != claimed.definition_checksum()
                || original.request_checksum() != claimed.request_checksum()
                || original.attempt().runner_id != claimed.attempt().runner_id
                || existing.preparation.is_some()
                || existing.ports.is_some()
                || existing.receipt.is_some()
                || existing.unsubmitted.is_some()
                || existing.settled
            {
                return Err(invalid());
            }
            return Ok(existing.admission.clone());
        }
        let host = Arc::new(MontyTaskHost::new_private(Arc::new(claimed.clone())));
        let retention = self.retention.retain(&host).map_err(|_| invalid())?;
        let admission = Arc::new(
            PgMontyAdmission::prepare_private(self.pool.clone(), Arc::new(claimed.clone()))
                .map_err(|_| invalid())?,
        );
        tasks.insert(
            claimed.attempt(),
            RetainedTask {
                host,
                retention,
                admission: admission.clone(),
                preparation: None,
                ports: None,
                receipt: None,
                execution_report: None,
                settled: false,
                provider_watch: None,
                provider_port: None,
                unsubmitted: None,
            },
        );
        Ok(admission)
    }
}
