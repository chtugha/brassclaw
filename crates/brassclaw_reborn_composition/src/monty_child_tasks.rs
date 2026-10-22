//! Native child admission against the parent's retained catalogue. Monty owns
//! workflow selection and sequencing; this owner supplies transport, lineage,
//! correlation and durable observation through the existing turn coordinator.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use brassclaw_host_api::{InvocationId, JsonValue, ResourceScope, ThreadId};
use brassclaw_host_runtime::{
    RuntimeChildTaskError as Error, RuntimeChildTaskOutcome, RuntimeChildTaskPort,
    RuntimeChildTaskReply, RuntimeChildTaskRequest,
};
use brassclaw_monty_host::service::{TaskInput, TaskPorts};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_skills::value_contract::ContractLimits;
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, FinalizedAssistantMessageByRunRequest,
    MessageContent, SessionThreadService, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, CancelRunRequest, GetRunStateRequest, IdempotencyKey, LoopMessageRef,
    ReplyTargetBindingRef, RunProfileRequest, SanitizedCancelReason, SourceBindingRef,
    SubmitChildRunRequest, SubmitTurnResponse, TurnCoordinator, TurnError, TurnEventSink,
    TurnLifecycleEvent, TurnRunId, TurnScope, TurnSpawnTreePort, TurnSpawnTreeStateStore,
    TurnStatus,
    run_profile::{AgentLoopDriverError, MontyTaskAttempt},
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{InstalledMontyCatalogue, TaskCatalogue, copy_selection, failed};
use crate::{
    RebornBuildError, global_recipe_ports::SelectedMontyRecipe,
    global_task_factory::CapturedChildRecipe, pg_monty_admission::PgMontyAdmission,
};

#[path = "monty_child_admission.rs"]
mod admission_reconciliation;

const SOURCE_PREFIX: &str = "monty-child-v1:";

pub(super) struct ChildRuntime {
    coordinator: Arc<dyn TurnCoordinator>,
    admission: Arc<dyn TurnSpawnTreePort>,
    state: Arc<dyn TurnSpawnTreeStateStore>,
    threads: Arc<dyn SessionThreadService>,
}

struct ParentPorts {
    host: Weak<MontyTaskHost>,
    admission: Weak<PgMontyAdmission>,
    ports: Weak<dyn TaskPorts>,
}

#[derive(Default)]
pub(super) struct ChildTasks {
    runtime: OnceLock<ChildRuntime>,
    parents: Mutex<HashMap<MontyTaskAttempt, ParentPorts>>,
    operations: Mutex<HashMap<TurnRunId, Arc<ChildOperation>>>,
    #[cfg(test)]
    next_gate: Mutex<Option<Arc<ChildGate>>>,
}

#[cfg(test)]
struct ChildGate {
    captured: Arc<tokio::sync::Notify>,
    waiting: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

// No Debug/serde: claims, catalogue custody and complete inputs stay private.
struct ChildOperation {
    parent: Arc<MontyTaskHost>,
    parent_admission: Arc<PgMontyAdmission>,
    view: Arc<crate::authored_monty_catalogue::CatalogueGeneration>,
    workflow: Uuid,
    variant: String,
    inputs: JsonValue,
    run: TurnRunId,
    scope: TurnScope,
    reference: String,
    invocation: InvocationId,
    received_at: chrono::DateTime<chrono::Utc>,
    dependency_parent: Weak<dyn TaskPorts>,
    child_admission: Mutex<Option<Arc<PgMontyAdmission>>>,
    observed: OnceLock<RuntimeChildTaskOutcome>,
    changed: tokio::sync::Notify,
    reservation_acknowledged: AtomicBool,
    terminal_observed: AtomicBool,
    #[cfg(test)]
    gate: Option<Arc<ChildGate>>,
}

impl ChildTasks {
    pub(super) fn observe(
        &self,
        host: &Arc<MontyTaskHost>,
        admission: &Arc<PgMontyAdmission>,
        ports: Weak<dyn TaskPorts>,
    ) -> Result<(), AgentLoopDriverError> {
        let mut parents = self.parents.lock().map_err(|_| failed())?;
        parents.retain(|_, entry| entry.host.strong_count() > 0);
        if parents.contains_key(&host.attempt()) {
            return Err(failed());
        }
        parents.try_reserve(1).map_err(|_| failed())?;
        parents.insert(
            host.attempt(),
            ParentPorts {
                host: Arc::downgrade(host),
                admission: Arc::downgrade(admission),
                ports,
            },
        );
        Ok(())
    }

    pub(super) fn release(&self, attempt: MontyTaskAttempt) -> Result<(), AgentLoopDriverError> {
        // Unknown child outcomes keep their original private address/view.
        // Releasing the parent never declares those external effects resolved.
        self.operations
            .lock()
            .map_err(|_| failed())?
            .retain(|_, child| {
                child.parent.attempt() != attempt
                    || !child.terminal_observed.load(Ordering::Acquire)
            });
        self.parents.lock().map_err(|_| failed())?.remove(&attempt);
        Ok(())
    }
}

impl InstalledMontyCatalogue {
    /// Evidence acknowledgement only. An admission without an actually
    /// received terminal result remains unknown with its original owner.
    pub(super) async fn acknowledge_received_child_results(
        &self,
        attempt: MontyTaskAttempt,
    ) -> Result<(), AgentLoopDriverError> {
        let received = self
            .child_tasks
            .operations
            .lock()
            .map_err(|_| child_result_pending())?
            .values()
            .filter(|operation| {
                operation.parent.attempt() == attempt
                    && operation.observed.get().is_some()
                    && !operation.reservation_acknowledged.load(Ordering::Acquire)
            })
            .cloned()
            .collect::<Vec<_>>();
        if received.is_empty() {
            return Ok(());
        }
        let runtime = self
            .child_tasks
            .runtime
            .get()
            .ok_or_else(child_result_pending)?;
        for operation in received {
            acknowledge_child_observation(runtime, &operation)
                .await
                .map_err(|_| child_result_pending())?;
        }
        Ok(())
    }

    /// Barriers observe actual capture and a real nonterminal durable read;
    /// they cannot supply a child result or change admitted identities.
    #[cfg(test)]
    pub(crate) fn pause_next_child_for_test(
        &self,
    ) -> (
        Arc<tokio::sync::Notify>,
        Arc<tokio::sync::Notify>,
        Arc<tokio::sync::Notify>,
    ) {
        let gate = Arc::new(ChildGate {
            captured: Arc::new(tokio::sync::Notify::new()),
            waiting: Arc::new(tokio::sync::Notify::new()),
            release: Arc::new(tokio::sync::Notify::new()),
        });
        let mut pending = self
            .child_tasks
            .next_gate
            .lock()
            .expect("test barriers available");
        assert!(pending.is_none());
        *pending = Some(gate.clone());
        (
            gate.captured.clone(),
            gate.waiting.clone(),
            gate.release.clone(),
        )
    }

    /// Lifecycle events are wake hints only. Waiters always reread the durable
    /// child state; a lost publication retains the configured polling fallback.
    pub(crate) fn child_event_sink(self: &Arc<Self>) -> Arc<dyn TurnEventSink> {
        Arc::new(ChildEvents {
            owner: Arc::downgrade(self),
        })
    }

    #[cfg(test)]
    pub(crate) fn retained_child_operations_for_test(&self, parent: MontyTaskAttempt) -> usize {
        self.child_tasks
            .operations
            .lock()
            .expect("child observations available")
            .values()
            .filter(|operation| operation.parent.attempt() == parent)
            .count()
    }

    /// Inspect the original owner in contained native acceptance. This cannot
    /// reconstruct an address, manufacture a claim or register another parent.
    #[cfg(test)]
    pub(crate) fn retained_child_parent_for_test(
        &self,
        host: &Arc<MontyTaskHost>,
    ) -> Result<Arc<PgMontyAdmission>, AgentLoopDriverError> {
        let parents = self.child_tasks.parents.lock().map_err(|_| failed())?;
        let parent = parents.get(&host.attempt()).ok_or_else(failed)?;
        if !parent
            .host
            .upgrade()
            .is_some_and(|original| Arc::ptr_eq(&original, host))
        {
            return Err(failed());
        }
        parent.admission.upgrade().ok_or_else(failed)
    }

    pub(crate) fn bind_child_runtime(
        &self,
        coordinator: Arc<dyn TurnCoordinator>,
        admission: Arc<dyn TurnSpawnTreePort>,
        state: Arc<dyn TurnSpawnTreeStateStore>,
        threads: Arc<dyn SessionThreadService>,
    ) -> Result<(), RebornBuildError> {
        self.child_tasks
            .runtime
            .set(ChildRuntime {
                coordinator,
                admission,
                state,
                threads,
            })
            .map_err(|_| super::invalid("child runtime already bound"))
    }

    pub(crate) fn child_task_port(
        self: &Arc<Self>,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
    ) -> Arc<dyn RuntimeChildTaskPort> {
        Arc::new(ChildPort {
            owner: Arc::downgrade(self),
            host,
            admission,
        })
    }

    async fn child_data_limits(&self) -> Result<ContractLimits, Error> {
        let policy = self
            .preparation_policy()
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(ContractLimits {
            max_depth: usize::try_from(policy.max_value_depth).map_err(|_| Error::Unavailable)?,
            max_nodes: usize::try_from(policy.max_value_nodes).map_err(|_| Error::Unavailable)?,
            max_bytes: usize::try_from(policy.max_value_bytes).map_err(|_| Error::Unavailable)?,
        })
    }

    pub(super) async fn capture_child_task(
        self: &Arc<Self>,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Option<CapturedChildRecipe>, AgentLoopDriverError> {
        let Some(claim) = host.claimed_run() else {
            // Historical/contained validation hosts supply no child authority.
            return Ok(None);
        };
        if !claim
            .state
            .source_binding_ref
            .as_str()
            .starts_with(SOURCE_PREFIX)
        {
            return Ok(None);
        }
        let operation = self
            .child_tasks
            .operations
            .lock()
            .map_err(|_| failed())?
            .get(&host.attempt().run_id)
            .cloned()
            .ok_or_else(failed)?;
        let runtime = self.child_tasks.runtime.get().ok_or_else(failed)?;
        let record = runtime
            .state
            .get_run_record(&operation.scope, operation.run)
            .await
            .map_err(|_| failed())?
            .ok_or_else(failed)?;
        let parent_claim = operation.parent.claimed_run().ok_or_else(failed)?;
        // Explicitly requesting the same profile can change resolver provenance
        // and its request fingerprint. Every execution field must still match
        // the parent's retained profile; these two audit labels grant nothing.
        let mut expected_profile = parent_claim.resolved_run_profile.clone();
        expected_profile.provenance = claim.resolved_run_profile.provenance.clone();
        expected_profile.resolution_fingerprint =
            claim.resolved_run_profile.resolution_fingerprint.clone();
        if operation.scope != claim.state.scope
            || operation.run != claim.state.run_id
            || operation.reference != input.user_input
            || operation.reference != claim.state.source_binding_ref.as_str()
            || operation.reference != claim.state.reply_target_binding_ref.as_str()
            || claim.state.actor != parent_claim.state.actor
            || claim.resolved_run_profile != expected_profile
            || record.parent_run_id != Some(parent_claim.state.run_id)
            || record.subagent_depth == 0
            || record.claimed_child_reservation_released != Some(false)
            || record.runner_id != Some(claim.runner_id)
            || record.lease_token != Some(claim.lease_token)
        {
            return Err(failed());
        }
        operation
            .parent
            .check_dispatch_open()
            .map_err(|_| failed())?;
        operation.parent_admission.check_live_dependency().await?;
        let recorded_at = operation
            .parent_admission
            .verify_child_invocation(
                operation.invocation,
                operation.workflow,
                &operation.variant,
                operation.inputs.as_value(),
                crate::pg_monty_admission::CatalogueReference {
                    id: operation.view.generation,
                    checksum: Sha256::digest(operation.view.bytes.as_bytes()).into(),
                },
            )
            .await?;
        if recorded_at != operation.received_at {
            return Err(failed());
        }
        #[cfg(test)]
        if let Some(gate) = &operation.gate {
            gate.captured.notify_one();
            gate.release.notified().await;
        }
        let dependency_parent = operation.dependency_parent.upgrade().ok_or_else(failed)?;
        // Only the original captured view is verified; never check current
        // active/latest here or fabricate an intent resolution for a child.
        let mut client = self.pool.get().await.map_err(|_| failed())?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failed())?;
        self.verify_retained_selection(&tx, &operation.view).await?;
        tx.commit().await.map_err(|_| failed())?;
        drop(client);
        let catalogue = self.bind_captured_view(
            host,
            admission.clone(),
            operation.view.clone(),
            None,
            input.user_input.clone(),
        )?;
        let selected = catalogue.select_explicit(
            operation.workflow,
            &operation.variant,
            operation.inputs.as_value(),
            self.child_data_limits().await.map_err(|_| failed())?,
        )?;
        operation
            .parent
            .check_dispatch_open()
            .map_err(|_| failed())?;
        operation.parent_admission.check_live_dependency().await?;
        // Retain before the caller starts this child. A second captured claim
        // is recovery work, never another attempt of the whole Recipe.
        let mut bound = operation.child_admission.lock().map_err(|_| failed())?;
        if bound.is_some() {
            return Err(failed());
        }
        *bound = Some(admission);
        Ok(Some(CapturedChildRecipe {
            catalogue,
            selected,
            dependency_parent,
        }))
    }
}

impl TaskCatalogue {
    fn select_explicit(
        &self,
        recipe: Uuid,
        variant: &str,
        supplied: &serde_json::Value,
        limits: ContractLimits,
    ) -> Result<SelectedMontyRecipe, AgentLoopDriverError> {
        // Ordinary child tasks use the same public workflow set as chat
        // routing and must publish their own correlated reply. The retained
        // history helper is internal finalization, not a child task goal.
        let mut choices = std::iter::once(&self.reply)
            .chain(self.incoming.iter())
            .filter(|selected| {
                let instruction = selected.inspected.program().inputs().instruction();
                instruction.recipe().uuid == recipe
                    && instruction.variant().variant_key == variant
                    && self.view.routing_enabled(&selected.inspected)
            });
        let original = choices.next().ok_or_else(failed)?;
        if choices.next().is_some() {
            return Err(failed());
        }
        let mut selected = copy_selection(original);
        selected.inputs = Some(
            selected
                .inspected
                .program()
                .inputs()
                .bind_task_inputs_with_limits(supplied, limits)
                .map_err(|_| failed())?
                .into(),
        );
        Ok(selected)
    }
}

struct ChildPort {
    owner: Weak<InstalledMontyCatalogue>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
}

struct ChildEvents {
    owner: Weak<InstalledMontyCatalogue>,
}
#[async_trait]
impl TurnEventSink for ChildEvents {
    async fn publish(&self, event: TurnLifecycleEvent) -> Result<(), TurnError> {
        let Some(owner) = self.owner.upgrade() else {
            return Ok(());
        };
        let operation = owner
            .child_tasks
            .operations
            .lock()
            .map_err(|_| TurnError::Unavailable {
                reason: "child lifecycle observations unavailable".into(),
            })?
            .get(&event.run_id)
            .cloned();
        if let Some(operation) = operation {
            operation.changed.notify_waiters();
        }
        Ok(())
    }
}

// Namespace and version are part of durable child identity. One host-created
// native invocation always addresses one child, including unknown acknowledgements.
fn child_run(invocation: InvocationId) -> TurnRunId {
    let mut hash = Sha256::new();
    hash.update(b"brassclaw:monty-native-child:1\0");
    hash.update(invocation.as_uuid().as_bytes());
    let mut bytes: [u8; 16] = hash.finalize()[..16]
        .try_into()
        .expect("fixed digest width");
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    TurnRunId::from_uuid(Uuid::from_bytes(bytes))
}

fn thread_scope(scope: &TurnScope) -> Result<ThreadScope, Error> {
    Ok(ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id: scope.agent_id.clone().ok_or(Error::InvalidRequest)?,
        project_id: scope.project_id.clone(),
        owner_user_id: scope.explicit_owner_user_id().cloned(),
    })
}

#[async_trait]
impl RuntimeChildTaskPort for ChildPort {
    async fn delegate(
        &self,
        scope: &ResourceScope,
        request: RuntimeChildTaskRequest,
    ) -> Result<RuntimeChildTaskOutcome, Error> {
        let owner = self.owner.upgrade().ok_or(Error::Unavailable)?;
        let runtime = owner.child_tasks.runtime.get().ok_or(Error::Unavailable)?;
        let context = self.host.run_context();
        let parent = self.host.claimed_run().ok_or(Error::Fenced)?;
        let actor = parent.state.actor.as_ref().ok_or(Error::Fenced)?;
        if scope.tenant_id != context.scope.tenant_id
            || scope.agent_id != context.scope.agent_id
            || scope.project_id != context.scope.project_id
            || scope.thread_id.as_ref() != Some(&context.thread_id)
            || scope.user_id != actor.user_id
        {
            return Err(Error::Fenced);
        }
        self.host.check_dispatch_open().map_err(|_| Error::Fenced)?;
        let captured = owner
            .captured_prefixes
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(&context.run_id.to_string())
            .and_then(Weak::upgrade)
            .ok_or(Error::Fenced)?;
        // Validate the exact workflow and recursive inputs before any child
        // thread/admission effect. Parent ports are never reused by the child.
        captured
            .select_explicit(
                request.workflow().recipe(),
                request.workflow().variant(),
                request.inputs(),
                owner.child_data_limits().await?,
            )
            .map_err(|_| Error::InvalidRequest)?;
        let received_at = self
            .admission
            .verify_child_invocation(
                scope.invocation_id,
                request.workflow().recipe(),
                request.workflow().variant(),
                request.inputs(),
                captured.reference,
            )
            .await
            .map_err(|_| Error::Fenced)?;
        let dependency_parent = {
            let parents = owner
                .child_tasks
                .parents
                .lock()
                .map_err(|_| Error::Unavailable)?;
            let entry = parents.get(&self.host.attempt()).ok_or(Error::Fenced)?;
            if !entry
                .host
                .upgrade()
                .is_some_and(|host| Arc::ptr_eq(&host, &self.host))
            {
                return Err(Error::Fenced);
            }
            if !entry
                .admission
                .upgrade()
                .is_some_and(|admission| Arc::ptr_eq(&admission, &self.admission))
            {
                return Err(Error::Fenced);
            }
            if entry.ports.strong_count() == 0 {
                return Err(Error::Fenced);
            }
            entry.ports.clone()
        };
        let run = child_run(scope.invocation_id);
        let reference = format!("{SOURCE_PREFIX}{}", scope.invocation_id);
        let mut child_scope = context.scope.clone();
        child_scope.thread_id =
            ThreadId::new(format!("monty-child-v1-{}", run)).map_err(|_| Error::InvalidRequest)?;
        let (workflow, inputs) = request.into_parts();
        let operation = Arc::new(ChildOperation {
            parent: self.host.clone(),
            parent_admission: self.admission.clone(),
            view: captured.view.clone(),
            workflow: workflow.recipe(),
            variant: workflow.variant().to_owned(),
            inputs,
            run,
            scope: child_scope,
            reference,
            invocation: scope.invocation_id,
            received_at,
            dependency_parent,
            child_admission: Mutex::new(None),
            observed: OnceLock::new(),
            changed: tokio::sync::Notify::new(),
            reservation_acknowledged: AtomicBool::new(false),
            terminal_observed: AtomicBool::new(false),
            #[cfg(test)]
            gate: owner
                .child_tasks
                .next_gate
                .lock()
                .map_err(|_| Error::Unavailable)?
                .take(),
        });
        {
            let mut operations = owner
                .child_tasks
                .operations
                .lock()
                .map_err(|_| Error::Unavailable)?;
            if operations.contains_key(&run) {
                return Err(Error::Uncertain);
            }
            operations.try_reserve(1).map_err(|_| Error::Unavailable)?;
            operations.insert(run, operation.clone());
        }
        // Retention precedes every write. Unknown commits keep this operation,
        // original IDs and inputs; never compensate by deleting accepted work.
        self.host.check_dispatch_open().map_err(|_| Error::Fenced)?;
        self.admission
            .check_live_dependency()
            .await
            .map_err(|_| Error::Fenced)?;
        let child_thread_scope = thread_scope(&operation.scope)?;
        let ensured = runtime
            .threads
            .ensure_thread(EnsureThreadRequest {
                scope: child_thread_scope.clone(),
                thread_id: Some(operation.scope.thread_id.clone()),
                created_by_actor_id: actor.user_id.to_string(),
                title: None,
                metadata_json: None,
            })
            .await;
        if ensured.is_err() {
            admission_reconciliation::ensured_thread(runtime, &operation).await?;
        }
        self.host.check_dispatch_open().map_err(|_| Error::Fenced)?;
        let accepted = runtime
            .threads
            .accept_inbound_message(AcceptInboundMessageRequest {
                scope: child_thread_scope.clone(),
                thread_id: operation.scope.thread_id.clone(),
                actor_id: actor.user_id.to_string(),
                source_binding_id: Some(operation.reference.clone()),
                reply_target_binding_id: Some(operation.reference.clone()),
                external_event_id: Some(operation.reference.clone()),
                content: MessageContent::text(&operation.reference),
            })
            .await;
        let message_id = match accepted {
            Ok(message) => message.message_id,
            Err(_) => admission_reconciliation::accepted_input(runtime, &operation).await?,
        };
        self.host.check_dispatch_open().map_err(|_| Error::Fenced)?;
        let submitted = runtime
            .admission
            .submit_claimed_child_run(
                SubmitChildRunRequest {
                    parent_scope: context.scope.clone(),
                    parent_run_id: context.run_id,
                    child_scope: operation.scope.clone(),
                    actor: actor.clone(),
                    accepted_message_ref: AcceptedMessageRef::new(format!("msg:{message_id}"))
                        .map_err(|_| Error::InvalidRequest)?,
                    source_binding_ref: SourceBindingRef::new(&operation.reference)
                        .map_err(|_| Error::InvalidRequest)?,
                    reply_target_binding_ref: ReplyTargetBindingRef::new(&operation.reference)
                        .map_err(|_| Error::InvalidRequest)?,
                    requested_run_profile: Some(
                        RunProfileRequest::new(context.resolved_run_profile.profile_id.as_str())
                            .map_err(|_| Error::InvalidRequest)?,
                    ),
                    idempotency_key: IdempotencyKey::new(&operation.reference)
                        .map_err(|_| Error::InvalidRequest)?,
                    received_at: operation.received_at,
                    requested_run_id: Some(operation.run),
                    // No additional policy quota: this is the store's u32 lineage
                    // representation bound. Instance resource admission still applies.
                    spawn_tree_descendant_cap: u32::MAX,
                },
                parent,
            )
            .await;
        let (turn_id, run_id) = match submitted {
            Ok(SubmitTurnResponse::Accepted {
                turn_id, run_id, ..
            }) => (turn_id, run_id),
            Err(_) => {
                // A lost COMMIT acknowledgement is not a failed admission.
                // Observe only the original, fully linked durable child; a
                // missing/mismatched result remains owned and uncertain.
                admission_reconciliation::committed_child(runtime, &operation, message_id).await?
            }
        };
        if run_id != operation.run {
            return Err(Error::Uncertain);
        }
        runtime
            .threads
            .mark_message_submitted(
                &child_thread_scope,
                &operation.scope.thread_id,
                message_id,
                turn_id.to_string(),
                run_id.to_string(),
            )
            .await
            .map_err(|_| Error::Uncertain)?;
        wait_for_child(&owner, runtime, &operation).await
    }
}

async fn wait_for_child(
    owner: &InstalledMontyCatalogue,
    runtime: &ChildRuntime,
    operation: &ChildOperation,
) -> Result<RuntimeChildTaskOutcome, Error> {
    let mut cancellation_sent = false;
    loop {
        // Arm before reading: a completion committed between the read and
        // await cannot be lost. Notifications themselves never prove success.
        let changed = operation.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        let state = runtime
            .coordinator
            .get_run_state(GetRunStateRequest {
                scope: operation.scope.clone(),
                run_id: operation.run,
            })
            .await
            .map_err(|_| Error::Uncertain)?;
        if state.status.is_terminal() {
            let outcome = match state.status {
                TurnStatus::Completed => {
                    let message = runtime
                        .threads
                        .finalized_assistant_message_by_run(FinalizedAssistantMessageByRunRequest {
                            scope: thread_scope(&operation.scope)?,
                            thread_id: operation.scope.thread_id.clone(),
                            turn_run_id: operation.run.to_string(),
                        })
                        .await
                        .map_err(|_| Error::Uncertain)?
                        .ok_or(Error::Uncertain)?;
                    let reference = format!("msg:{}", message.message_id);
                    let client = owner.pool.get().await.map_err(|_| Error::Uncertain)?;
                    let row = client.query_opt(
                        "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                        &[&operation.run.as_uuid()],
                    ).await.map_err(|_| Error::Uncertain)?.ok_or(Error::Uncertain)?;
                    let evidence = JsonValue::from(row.get::<_, serde_json::Value>(0));
                    if evidence["status"] != "completed"
                        || evidence["execution"]["all_selected_recipes_complete"] != true
                        || evidence["execution"]["reply"]["reference"] != reference
                    {
                        return Err(Error::Uncertain);
                    }
                    RuntimeChildTaskOutcome::Completed(RuntimeChildTaskReply {
                        child_run_id: operation.run,
                        thread_id: operation.scope.thread_id.clone(),
                        reply_ref: LoopMessageRef::new(reference).map_err(|_| Error::Uncertain)?,
                        content: message.content.ok_or(Error::Uncertain)?.into(),
                    })
                }
                TurnStatus::Failed => RuntimeChildTaskOutcome::Failed {
                    child_run_id: operation.run,
                    thread_id: operation.scope.thread_id.clone(),
                    failure: state.failure,
                },
                TurnStatus::Cancelled => RuntimeChildTaskOutcome::Cancelled {
                    child_run_id: operation.run,
                    thread_id: operation.scope.thread_id.clone(),
                },
                TurnStatus::RecoveryRequired => RuntimeChildTaskOutcome::RecoveryRequired {
                    child_run_id: operation.run,
                    thread_id: operation.scope.thread_id.clone(),
                },
                _ => return Err(Error::Uncertain),
            };
            // Keep the actual received terminal result before returning to
            // native resource/journal finalization, which can still fail.
            operation
                .observed
                .set(outcome.clone())
                .map_err(|_| Error::Uncertain)?;
            acknowledge_child_observation(runtime, operation).await?;
            return Ok(outcome);
        }
        #[cfg(test)]
        if let Some(gate) = &operation.gate {
            gate.waiting.notify_one();
        }
        if !cancellation_sent && operation.parent.check_dispatch_open().is_err() {
            let actor = operation
                .parent
                .run_context()
                .actor
                .clone()
                .ok_or(Error::Fenced)?;
            runtime
                .coordinator
                .cancel_tree(CancelRunRequest {
                    scope: operation.scope.clone(),
                    actor,
                    run_id: operation.run,
                    reason: SanitizedCancelReason::UserRequested,
                    idempotency_key: IdempotencyKey::new(format!("{}:cancel", operation.reference))
                        .map_err(|_| Error::InvalidRequest)?,
                })
                .await
                .map_err(|_| Error::Uncertain)?;
            cancellation_sent = true;
        }
        let policy = owner
            .preparation_policy()
            .await
            .map_err(|_| Error::Unavailable)?;
        let pause = tokio::time::sleep(Duration::from_millis(u64::from(
            policy.status_poll_interval_millis,
        )));
        if cancellation_sent {
            tokio::select! { _ = pause => {}, _ = changed => {} }
        } else {
            tokio::select! {
                _ = pause => {},
                _ = changed => {},
                _ = operation.parent.cancellation_requested() => {},
            }
        }
    }
}

async fn acknowledge_child_observation(
    runtime: &ChildRuntime,
    operation: &ChildOperation,
) -> Result<(), Error> {
    let outcome = operation.observed.get().ok_or(Error::Uncertain)?;
    runtime
        .state
        .release_claimed_child_reservation(&operation.scope, operation.run)
        .await
        .map_err(|_| Error::Uncertain)?;
    operation
        .reservation_acknowledged
        .store(true, Ordering::Release);
    if matches!(
        outcome,
        RuntimeChildTaskOutcome::Completed(_) | RuntimeChildTaskOutcome::Cancelled { .. }
    ) {
        operation.terminal_observed.store(true, Ordering::Release);
    }
    Ok(())
}

fn child_result_pending() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "monty_child_result_acknowledgement_required".into(),
    }
}
