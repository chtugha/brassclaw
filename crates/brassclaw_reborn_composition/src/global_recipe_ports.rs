//! Task-owned Recipe transport for the global orchestrator cutover.
//!
//! Catalogue selection and approval stay with the catalogue owner. This adapter
//! retains its exact inspected selection, exposes only opaque program references
//! and executes exactly the step requested by Python. It supplies no activation,
//! matching fallback, automatic retry or Rust workflow loop.
use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{RetainedRecipeExecution, RetainedTaskContext, RetainedToolPort},
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{
    process::TaskHandle,
    service::{PortFailure, TaskOutcome, TaskPorts, TaskReceipt},
    transport_actor::TransportClient,
};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use futures::{FutureExt, future::BoxFuture};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::pg_monty_admission::{CatalogueReference, PgMontyAdmission};

/// A catalogue-owned selection, not a caller-supplied approval flag. Production
/// owners must establish exact activation/review/artifact provenance before
/// returning it. Draft behavioral validation uses a separate owner.
pub(crate) struct SelectedMontyRecipe {
    pub(crate) normal_match: Option<Box<crate::normal_match_evidence::NormalMatchEvidence>>,
    pub(crate) inspected: Arc<InspectedRetainedProgram>,
    pub(crate) inputs: Option<Value>,
    pub(crate) tools: Option<Arc<dyn RetainedToolPort>>,
}

pub(crate) enum MontyIntentSelection {
    NoMatch,
    Disambiguation,
    Match(SelectedMontyRecipe),
}

/// One retained catalogue view per admitted task. Only activated, approved
/// components belong to its routing catalogue; drafts are excluded before
/// matching. Match/NoMatch are routing outcomes, not implementation support
/// categories. A selected workflow must have complete runner support. Technical
/// read/assembly/execution errors never manufacture NoMatch or replay Tier 2.
/// Implementations use the same generation for matching, named lookup, review
/// and IBS; none may reread latest after selection. Selection executes nothing.
#[async_trait]
pub(crate) trait MontyTaskCatalogue: Send + Sync {
    /// Actual owner-selected ordinary generation. Draft validation has no
    /// activated catalogue; absence never means "use latest" or grants approval.
    fn catalogue_reference(&self) -> Option<CatalogueReference> {
        None
    }
    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure>;
    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure>;
    async fn refresh_command_qualification(&self);
}

/// Host-owned private work selection. Implementations pin an approved program
/// for production; closed pre-readiness qualification uses its separate observer
/// constructor. Neither a name nor a receipt string grants approval or Tool use.
#[async_trait]
pub(crate) trait InternalRecipeSource: Send + Sync {
    fn name(&self) -> &str;
    fn reference(&self) -> &str;
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure>;
    /// Check actual retained native effects/answers and correlated durable rows.
    /// This may read evidence, but must never replay effects or select next work.
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure>;
}

/// An acknowledged instance resource policy, independent of task selection and
/// component approval. Reading it grants no Tool authority.
#[derive(Clone, Copy)]
pub(crate) struct RecipeCapacity {
    pub(crate) revision: u64,
    pub(crate) max_recipes: usize,
    pub(crate) data_limits: brassclaw_skills::value_contract::ContractLimits,
}
#[async_trait]
pub(crate) trait RecipeCapacitySource: Send + Sync {
    async fn current(&self) -> Result<RecipeCapacity, PortFailure>;
}

pub(super) struct Selection {
    selected: SelectedMontyRecipe,
    program_ref: String,
    pub(super) execution: Mutex<Option<RetainedRecipeExecution>>,
}
#[derive(Default)]
pub(super) struct State {
    task: Option<TaskHandle>,
    intent_started: bool,
    // Errors retain this state; they are never reported as No-Match.
    pub(super) intent_outcome: Option<&'static str>,
    completed_reply: Option<(String, String)>,
    completed_receipt: Option<String>,
    pub(super) recipes: BTreeMap<Uuid, Arc<Selection>>,
    selection_order: Vec<Uuid>,
    checked_capacity: Option<RecipeCapacity>,
}

/// Private supervisor-owned state retains failed executions and their original
/// child/Tool answers. Service settlement must precede releasing this object.
pub(crate) struct GlobalRecipePorts {
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    catalogue: Option<Arc<dyn MontyTaskCatalogue>>,
    user_input: String,
    ownership: Option<crate::global_monty_owner::GlobalOwnerCheck>,
    internal: Option<Arc<dyn InternalRecipeSource>>,
    child: Option<Mutex<Option<SelectedMontyRecipe>>>,
    observe_behavior: bool,
    capacity: Arc<dyn RecipeCapacitySource>,
    pub(super) state: Mutex<State>,
    task_context: Mutex<Option<RetainedTaskContext>>,
    fenced: std::sync::atomic::AtomicBool,
    dependency_parent: Option<Arc<dyn TaskPorts>>,
}
impl GlobalRecipePorts {
    pub(crate) async fn refresh_command_qualification(&self) {
        if let Some(catalogue) = &self.catalogue {
            catalogue.refresh_command_qualification().await;
        }
    }
    #[cfg(test)]
    pub(crate) async fn matched_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        self.state
            .lock()
            .await
            .recipes
            .values()
            .find(|selection| selection.selected.normal_match.is_some())
            .map(|selection| selection.selected.inspected.clone())
    }
    pub(crate) fn new(
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        catalogue: Arc<dyn MontyTaskCatalogue>,
        user_input: String,
        ownership: crate::global_monty_owner::GlobalOwnerCheck,
        capacity: Arc<dyn RecipeCapacitySource>,
    ) -> Self {
        Self {
            host,
            admission,
            catalogue: Some(catalogue),
            user_input,
            ownership: Some(ownership),
            internal: None,
            child: None,
            observe_behavior: false,
            capacity,
            state: Mutex::new(State::default()),
            task_context: Mutex::new(None),
            fenced: std::sync::atomic::AtomicBool::new(false),
            dependency_parent: None,
        }
    }

    /// Production internal work retains the normal live instance owner/fence.
    /// Only a host-validated durable definition can supply the source.
    pub(crate) fn for_internal(
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        source: Arc<dyn InternalRecipeSource>,
        ownership: Option<crate::global_monty_owner::GlobalOwnerCheck>,
        observe_behavior: bool,
        capacity: Arc<dyn RecipeCapacitySource>,
    ) -> Self {
        Self {
            host,
            admission,
            catalogue: None,
            user_input: source.reference().to_owned(),
            ownership,
            internal: Some(source),
            child: None,
            observe_behavior,
            capacity,
            state: Mutex::new(State::default()),
            task_context: Mutex::new(None),
            fenced: std::sync::atomic::AtomicBool::new(false),
            dependency_parent: None,
        }
    }

    /// Only a trusted catalogue/admission owner supplies this selection. It
    /// retains fresh child ports against the parent's exact catalogue and must
    /// establish durable child provenance before the global driver calls here.
    pub(crate) fn with_child_recipe(
        mut self,
        selected: SelectedMontyRecipe,
    ) -> Result<Self, PortFailure> {
        if self.internal.is_some()
            || selected.normal_match.is_some()
            || selected
                .inputs
                .as_ref()
                .is_none_or(|inputs| !inputs.is_object())
        {
            return Err(failure("child_recipe_selection_invalid"));
        }
        self.child = Some(Mutex::new(Some(selected)));
        Ok(self)
    }

    pub(crate) fn with_dependency_parent(mut self, parent: Option<Arc<dyn TaskPorts>>) -> Self {
        self.dependency_parent = parent;
        self
    }

    async fn check_ownership(&self) -> Result<(), PortFailure> {
        if let Some(ownership) = &self.ownership {
            ownership
                .check()
                .await
                .map_err(|_| failure("monty_instance_ownership_failed"))?;
        }
        self.check_fence()
    }

    fn check_fence(&self) -> Result<(), PortFailure> {
        if self.fenced.load(std::sync::atomic::Ordering::Acquire) {
            return Err(failure("task_cancelled"));
        }
        Ok(())
    }

    /// Actual task/child lifecycle observations after service quiescence. No
    /// raw values, claims, transport handles, approval or success-write input.
    /// The factory retains these exact bytes before durable acknowledgement.
    pub(crate) async fn settlement_report(
        &self,
        receipt: &TaskReceipt,
    ) -> Result<Value, PortFailure> {
        let (task, recipes, capacity, intent_outcome, completed_reply, completed_receipt) = {
            let state = self.state.lock().await;
            (
                state.task,
                state
                    .selection_order
                    .iter()
                    .map(|id| state.recipes[id].clone())
                    .collect::<Vec<_>>(),
                state.checked_capacity,
                state.intent_outcome,
                state.completed_reply.clone(),
                state.completed_receipt.clone(),
            )
        };
        if task.is_some_and(|task| {
            receipt
                .accounting
                .as_ref()
                .is_none_or(|account| account.task != task)
        }) {
            return Err(failure("task_accounting_failed"));
        }
        let mut progress = Vec::new();
        let mut complete = true;
        for selection in recipes {
            if selection
                .selected
                .tools
                .as_ref()
                .is_some_and(|tools| tools.has_uncommitted_native_results())
            {
                return Err(failure("native_result_evidence_uncommitted"));
            }
            let selected = selection
                .selected
                .inspected
                .program()
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(|_| failure("recipe_composition_failed"))?;
            let execution = selection.execution.lock().await;
            let finished = execution
                .as_ref()
                .is_some_and(RetainedRecipeExecution::is_complete);
            complete &= finished;
            let checksum: String = selected
                .checksum()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            progress.push(json!({
                "recipe_id":selection.selected.inspected.program().inputs().instruction().recipe().uuid,
                "selection_checksum":checksum,
                "composed":execution.is_some(), "complete":finished,
                "completed_steps":execution.as_ref().map(|execution| execution.completed_step_ids().collect::<Vec<_>>()).unwrap_or_default(),
                "pending_step":execution.as_ref().and_then(RetainedRecipeExecution::pending_step_id),
                "failed_step":execution.as_ref().and_then(RetainedRecipeExecution::failed_step_id),
                "normal_match":selection.selected.normal_match.as_ref().map(|evidence| json!({
                    "matching":evidence,
                    "recipe_completed":finished,
                    "task_completed":matches!(receipt.outcome, TaskOutcome::Completed { .. }),
                })),
            }));
        }
        let completed = matches!(
            receipt.outcome,
            TaskOutcome::Completed { .. } | TaskOutcome::InternalCompleted { .. }
        );
        if let TaskOutcome::InternalCompleted { receipt_ref } = &receipt.outcome
            && (self.internal.is_none() || completed_receipt.as_ref() != Some(receipt_ref))
        {
            return Err(failure("internal_receipt_evidence_missing"));
        }
        if let TaskOutcome::Completed { reply_ref } = &receipt.outcome
            && completed_reply
                .as_ref()
                .is_none_or(|(reference, _)| reference != reply_ref)
        {
            return Err(failure("task_reply_evidence_missing"));
        }
        if completed
            && (!complete
                || task.is_none()
                || receipt.accounting.as_ref().is_none_or(|account| {
                    account.compute_time.is_none() || account.failure.is_some()
                }))
        {
            return Err(failure("task_completion_invalid"));
        }
        let hex = |checksum: [u8; 32]| {
            checksum
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let root = json!({"format":"monty-root-execution/1",
            "vm_id":receipt.root.vm_id(), "source_checksum":hex(receipt.root.source_checksum()),
            "aliases_checksum":hex(receipt.root.aliases_checksum()), "workers":receipt.root.workers()});
        let account = receipt.accounting.as_ref().map(|account| {
            json!({
                "effective_revision":account.effective_revision,
                "compute_time":account.compute_time, "failure":account.failure,
            })
        });
        let (model_exchanges, capability_dispatches) = self.host.review_dispatch_counts();
        let mut report = json!({"format":"monty-task-execution/1", "root":root, "root_completed":completed,
            "all_selected_recipes_complete":complete, "recipes":progress,
            "intent_outcome":intent_outcome.unwrap_or("not_started"),
            "reply":completed_reply.or_else(|| self.host.finalized_reply_evidence().map(|(reference, content)| {
                use sha2::Digest;
                (reference, hex::encode(sha2::Sha256::digest(content.as_bytes())))
            })).map(|(reference, checksum)| json!({"reference":reference,"content_checksum":checksum})),
            "checked_recipe_capacity":capacity.map(|capacity| json!({
                "revision":capacity.revision,"max_recipes":capacity.max_recipes})),
            "accounting":account, "withheld_root_answers":receipt.withheld.len(),
            "withheld_host_answers":self.host.has_withheld_results(),
            "model_exchange_count":model_exchanges,"capability_dispatch_count":capability_dispatches,
            "semantic_approval":false,"catalogue_activation":false});
        if self.internal.is_some() {
            report["format"] = json!("monty-internal-task-execution/1");
            report["internal_receipt"] = json!(completed_receipt);
        }
        Ok(report)
    }

    async fn retain_selection(
        &self,
        selected: SelectedMontyRecipe,
    ) -> Result<(Uuid, String), PortFailure> {
        let mut state = self.state.lock().await;
        // The settings source waits for complete acknowledgement. No catalogue
        // read, child feed or effect occurs while choosing the current bound.
        let capacity = self.capacity.current().await?;
        self.check_fence()?;
        retain(&mut state, selected, capacity)
    }

    async fn check_recipe_completion(&self) -> Result<(), PortFailure> {
        let selections: Vec<_> = self.state.lock().await.recipes.values().cloned().collect();
        for selection in selections {
            if !selection
                .execution
                .lock()
                .await
                .as_ref()
                .is_some_and(RetainedRecipeExecution::is_complete)
            {
                return Err(failure("recipe_execution_incomplete"));
            }
        }
        Ok(())
    }

    async fn dispatch(
        &self,
        task: TaskHandle,
        transport: &TransportClient,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, PortFailure> {
        self.check_fence()?;
        self.check_ownership().await?;
        self.admission
            .bind_catalogue(
                self.catalogue
                    .as_ref()
                    .and_then(|catalogue| catalogue.catalogue_reference()),
            )
            .map_err(|_| failure("monty_catalogue_reference_conflict"))?;
        self.admission
            .check_and_start()
            .await
            .map_err(|_| failure("monty_admission_fenced"))?;
        // Only this short task-local ownership check covers all host operations.
        // Model/provider waits do not hold the Recipe state lock.
        {
            let mut state = self.state.lock().await;
            if state.task.is_some_and(|actual| actual != task) {
                return Err(failure("task_identity_invalid"));
            }
            state.task = Some(task);
        }
        match name {
            "resolve_child_recipe" => {
                if !args.is_empty() || self.internal.is_some() {
                    return Err(failure("child_recipe_selection_invalid"));
                }
                let child = self
                    .child
                    .as_ref()
                    .ok_or_else(|| failure("child_recipe_unavailable"))?;
                {
                    let mut state = self.state.lock().await;
                    if state.intent_started {
                        return Err(failure("child_recipe_selection_repeated"));
                    }
                    state.intent_started = true;
                    state.intent_outcome = Some("child_error");
                }
                // Transfer the one task-local selection into execution. Do
                // not keep another complete input tree/Tool owner alive after
                // this handoff. A failed/abandoned selection is never restored.
                let selected = child
                    .lock()
                    .await
                    .take()
                    .ok_or_else(|| failure("child_recipe_selection_repeated"))?;
                let inputs = selected
                    .inputs
                    .clone()
                    .ok_or_else(|| failure("child_recipe_inputs_missing"))?;
                self.check_fence()?;
                let (id, step_link) = self.retain_selection(selected).await?;
                self.state.lock().await.intent_outcome = Some("child_recipe");
                Ok(
                    json!({"status":"child_recipe","component_id":id,"step_link":step_link,"inputs":inputs}),
                )
            }
            "resolve_internal_recipe" => {
                let source = self
                    .internal
                    .as_ref()
                    .ok_or_else(|| failure("internal_recipe_unavailable"))?;
                if args != vec![json!(source.name()), json!(source.reference())] {
                    return Err(failure("internal_recipe_identity_invalid"));
                }
                {
                    let mut state = self.state.lock().await;
                    if state.intent_started {
                        return Err(failure("internal_recipe_selection_repeated"));
                    }
                    state.intent_started = true;
                    state.intent_outcome = Some("internal_error");
                }
                let selected = source.select().await?;
                if selected.normal_match.is_some() {
                    return Err(failure("internal_recipe_selection_invalid"));
                }
                let inputs = selected
                    .inputs
                    .clone()
                    .ok_or_else(|| failure("internal_recipe_inputs_missing"))?;
                self.check_fence()?;
                let (id, step_link) = self.retain_selection(selected).await?;
                self.state.lock().await.intent_outcome = Some("internal_recipe");
                Ok(
                    json!({"status":"internal_recipe","component_id":id,"step_link":step_link,"inputs":inputs}),
                )
            }
            "resolve_intent" => {
                if self.internal.is_some() || self.child.is_some() {
                    return Err(failure("internal_recipe_routing_invalid"));
                }
                let [query] = args.as_slice() else {
                    return Err(failure("intent_resolution_failed"));
                };
                if query.as_str() != Some(self.user_input.as_str()) {
                    return Err(failure("intent_resolution_failed"));
                }
                {
                    let mut state = self.state.lock().await;
                    if state.intent_started {
                        return Err(failure("intent_resolution_failed"));
                    }
                    // An uncertain/failed selection cannot match again.
                    state.intent_started = true;
                    state.intent_outcome = Some("error");
                }
                let selected = self
                    .catalogue
                    .as_ref()
                    .ok_or_else(|| failure("intent_resolution_failed"))?
                    .resolve_intent(&self.user_input)
                    .await?;
                self.check_fence()?;
                match selected {
                    MontyIntentSelection::NoMatch
                        if self.host.run_context().trusted_internal_turn =>
                    {
                        self.state.lock().await.intent_outcome = Some("no_match");
                        Err(failure("non_match_instruction_unavailable"))
                    }
                    MontyIntentSelection::NoMatch => {
                        self.state.lock().await.intent_outcome = Some("no_match");
                        Ok(json!({"status":"no_match"}))
                    }
                    MontyIntentSelection::Disambiguation => {
                        self.state.lock().await.intent_outcome = Some("disambiguation");
                        Ok(json!({"status":"disambiguation"}))
                    }
                    MontyIntentSelection::Match(selected) => {
                        if selected.normal_match.is_none() {
                            return Err(failure("intent_evidence_missing"));
                        }
                        let (recipe_id, step_link) = self.retain_selection(selected).await?;
                        self.state.lock().await.intent_outcome = Some("match");
                        Ok(
                            json!({"status":"match", "component_id":recipe_id, "step_link":step_link}),
                        )
                    }
                }
            }
            "resolve_component_by_name" => {
                if self.internal.is_some() {
                    return Err(failure("internal_recipe_routing_invalid"));
                }
                let [name, class] = args.as_slice() else {
                    return Err(failure("recipe_composition_failed"));
                };
                let name = name
                    .as_str()
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| failure("recipe_composition_failed"))?;
                if class.as_i64() != Some(21) {
                    return Err(failure("recipe_composition_failed"));
                }
                let selected = self
                    .catalogue
                    .as_ref()
                    .ok_or_else(|| failure("recipe_composition_failed"))?
                    .resolve_named_recipe(name)
                    .await?;
                self.check_fence()?;
                let (id, step_link) = self.retain_selection(selected).await?;
                Ok(json!({"id":id,"step_link":step_link}))
            }
            "compose_orchestrator" => {
                let [recipe, link, envelope] = args.as_slice() else {
                    return Err(failure("recipe_composition_failed"));
                };
                if !envelope.is_object() {
                    return Err(failure("recipe_composition_failed"));
                }
                let id = recipe
                    .as_str()
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .ok_or_else(|| failure("recipe_composition_failed"))?;
                let selected = self
                    .state
                    .lock()
                    .await
                    .recipes
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| failure("recipe_composition_failed"))?;
                let mut execution_slot = selected.execution.lock().await;
                let program = selected.selected.inspected.program();
                let instruction = program.inputs().instruction();
                if link.as_str() != instruction.variant().step_link.as_deref()
                    || execution_slot.is_some()
                {
                    return Err(failure("recipe_composition_failed"));
                }
                let bounds = *transport.live_vm_bounds().borrow();
                let limits = brassclaw_skills::value_contract::ContractLimits {
                    max_depth: bounds.max_value_depth,
                    max_nodes: bounds.max_value_nodes,
                    max_bytes: bounds.max_value_bytes,
                };
                let inputs = program
                    .inputs()
                    .bind_task_inputs_with_limits(
                        selected.selected.inputs.as_ref().unwrap_or(envelope),
                        limits,
                    )
                    .map_err(|_| failure("recipe_composition_failed"))?;
                // Retain selection before any child feed, including pure logic.
                // A failed or uncertain commit cannot expose a program reference.
                self.admission
                    .retain_recipe_selection(instruction)
                    .await
                    .map_err(|_| failure("recipe_selection_persistence_failed"))?;
                self.check_fence()?;
                let flow = program
                    .inputs()
                    .monty_flow()
                    .map_err(|_| failure("recipe_composition_failed"))?;
                let steps: Vec<_> = program
                    .program()
                    .steplist
                    .iter()
                    .map(|step| json!({"step_id":step.step_id}))
                    .collect();
                let parent = {
                    // This per-task lock covers only its one parent allocation.
                    // Cancellation uses the atomic fence and never waits here.
                    let mut slot = self.task_context.lock().await;
                    let context = slot.get_or_insert_with(|| RetainedTaskContext::new(task));
                    if context.task() != task {
                        return Err(failure("task_identity_invalid"));
                    }
                    context
                        .open(transport)
                        .await
                        .map_err(|_| failure("recipe_context_failed"))?
                };
                self.check_fence()?;
                let execution = if self.observe_behavior {
                    RetainedRecipeExecution::new_child_for_behavioral_validation(
                        task,
                        parent,
                        selected.selected.inspected.clone(),
                    )
                } else {
                    RetainedRecipeExecution::new_child(
                        task,
                        parent,
                        selected.selected.inspected.clone(),
                    )
                }
                .map_err(|_| failure("recipe_composition_failed"))?;
                *execution_slot = Some(execution);
                Ok(
                    json!({"ok":true,"program_ref":selected.program_ref,"steps":steps,
                    "inputs":inputs,"flow":flow}),
                )
            }
            "run_program" => {
                let [reference, step, frame] = args.as_slice() else {
                    return Err(failure("recipe_execution_failed"));
                };
                let object = frame
                    .as_object()
                    .filter(|object| {
                        object.len() == 2
                            && object.contains_key("inputs")
                            && object.contains_key("occurrence")
                    })
                    .ok_or_else(|| failure("recipe_execution_failed"))?;
                // This adapter supports the compiler's flat stop-only flow.
                // Repetition/retry needs durable occurrence addressing first.
                let step = step
                    .as_str()
                    .ok_or_else(|| failure("recipe_execution_failed"))?;
                let selection = self
                    .state
                    .lock()
                    .await
                    .recipes
                    .values()
                    .find(|selection| reference.as_str() == Some(selection.program_ref.as_str()))
                    .cloned()
                    .ok_or_else(|| failure("recipe_execution_failed"))?;
                let program = selection.selected.inspected.program();
                let index = program
                    .program()
                    .steplist
                    .iter()
                    .position(|selected| selected.step_id == step)
                    .ok_or_else(|| failure("recipe_execution_failed"))?;
                if object["occurrence"] != json!([format!("execute_{index}")]) {
                    return Err(failure("recipe_execution_failed"));
                }
                // This mutex protects only this child's exclusive feed state.
                // Task lookup and synchronous cancellation never acquire it.
                let mut execution_slot = selection.execution.lock().await;
                let execution = execution_slot
                    .as_mut()
                    .ok_or_else(|| failure("recipe_execution_failed"))?;
                self.check_fence()?;
                let value = execution
                    .run_step(
                        transport,
                        step,
                        &object["inputs"],
                        selection.selected.tools.as_deref(),
                    )
                    .await
                    .map_err(|_| failure("recipe_execution_failed"))?;
                self.check_fence()?;
                Ok(json!({"ok":true,"return_value":value}))
            }
            // Root publication must use its retained reply Recipe. A changed
            // root cannot bypass binding, live Tool policy and invocation intent
            // by calling the lower transcript adapter as a transport port.
            "post_reply" => Err(failure("task_port_invalid")),
            _ => {
                if self.internal.is_some() {
                    return Err(failure("internal_recipe_port_invalid"));
                }
                let [payload] = args.as_slice() else {
                    return Err(failure("task_port_invalid"));
                };
                let payload = if name == "resolve_reply" {
                    json!({"reply_ref":payload})
                } else {
                    payload.clone()
                };
                self.host
                    .dispatch_port(name, payload)
                    .await
                    .map_err(|_| failure("task_port_failed"))
            }
        }
    }
}
fn retain(
    state: &mut State,
    selected: SelectedMontyRecipe,
    capacity: RecipeCapacity,
) -> Result<(Uuid, String), PortFailure> {
    let program = selected.inspected.program();
    if matches!(
        program,
        brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(_)
    ) && selected.tools.is_none()
    {
        return Err(failure("recipe_composition_failed"));
    }
    let instruction = program.inputs().instruction();
    let id = instruction.recipe().uuid;
    let link = instruction
        .variant()
        .step_link
        .clone()
        .ok_or_else(|| failure("recipe_composition_failed"))?;
    // Validate the complete typed task contract before exposing the selection.
    if let Some(inputs) = &selected.inputs {
        program
            .inputs()
            .bind_task_inputs_with_limits(inputs, capacity.data_limits)
            .map_err(|_| failure("recipe_composition_failed"))?;
    }
    if state.recipes.contains_key(&id) {
        return Err(failure("recipe_composition_failed"));
    }
    state.checked_capacity = Some(capacity);
    if capacity.max_recipes == 0 || capacity.max_recipes > i32::MAX as usize {
        return Err(failure("recipe_capacity_invalid"));
    }
    if state.recipes.len() >= capacity.max_recipes {
        return Err(failure("recipe_capacity_exceeded"));
    }
    state.recipes.insert(
        id,
        Arc::new(Selection {
            selected,
            program_ref: Uuid::new_v4().to_string(),
            execution: Mutex::new(None),
        }),
    );
    state.selection_order.push(id);
    Ok((id, link))
}
fn failure(reason: &str) -> PortFailure {
    PortFailure::new(reason).expect("static classified reason")
}
impl TaskPorts for GlobalRecipePorts {
    fn dependency_parent(&self) -> Option<Arc<dyn TaskPorts>> {
        self.dependency_parent.clone()
    }

    fn call(
        self: Arc<Self>,
        task: TaskHandle,
        transport: TransportClient,
        name: String,
        args: Vec<Value>,
        kwargs: BTreeMap<String, Value>,
    ) -> BoxFuture<'static, Result<Value, PortFailure>> {
        async move {
            if !kwargs.is_empty() {
                return Err(failure("task_port_invalid"));
            }
            self.dispatch(task, &transport, &name, args).await
        }
        .boxed()
    }
    fn finish(
        self: Arc<Self>,
        outcome: TaskOutcome,
    ) -> BoxFuture<'static, Result<TaskOutcome, PortFailure>> {
        async move {
            if let TaskOutcome::InternalCompleted { receipt_ref } = &outcome {
                self.check_ownership().await?;
                let source = self
                    .internal
                    .as_ref()
                    .ok_or_else(|| failure("recipe_reply_invalid"))?;
                let selection = {
                    let state = self.state.lock().await;
                    if state.recipes.len() != 1
                        || state.intent_outcome != Some("internal_recipe")
                        || state.completed_receipt.is_some()
                    {
                        return Err(failure("internal_recipe_completion_invalid"));
                    }
                    state
                        .recipes
                        .values()
                        .next()
                        .cloned()
                        .ok_or_else(|| failure("internal_recipe_completion_invalid"))?
                };
                let execution = selection.execution.lock().await;
                let execution = execution
                    .as_ref()
                    .filter(|execution| execution.is_complete())
                    .ok_or_else(|| failure("recipe_execution_incomplete"))?;
                source
                    .verify_receipt(receipt_ref, &selection.selected.inspected, execution)
                    .await?;
                self.check_ownership().await?;
                self.state.lock().await.completed_receipt = Some(receipt_ref.clone());
                return Ok(outcome);
            }
            if self.internal.is_some() && matches!(outcome, TaskOutcome::Completed { .. }) {
                return Err(failure("internal_recipe_completion_invalid"));
            }
            if let TaskOutcome::Completed { reply_ref } = &outcome {
                self.check_fence()?;
                self.check_recipe_completion().await?;
                let reference = serde_json::from_value(Value::String(reply_ref.clone()))
                    .map_err(|_| failure("recipe_reply_invalid"))?;
                let content = self
                    .host
                    .published_reply_content(&reference)
                    .map_err(|_| failure("recipe_reply_invalid"))?;
                use sha2::Digest;
                self.state.lock().await.completed_reply = Some((
                    reply_ref.clone(),
                    hex::encode(sha2::Sha256::digest(content.as_bytes())),
                ));
            }
            Ok(outcome)
        }
        .boxed()
    }
    fn fence(&self) {
        self.fenced
            .store(true, std::sync::atomic::Ordering::Release);
        self.host.fence_dispatch();
    }
}
