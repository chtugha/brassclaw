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
    service::{PortFailure, TaskOutcome, TaskPorts},
    transport_actor::TransportClient,
};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use futures::{FutureExt, future::BoxFuture};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::pg_monty_admission::PgMontyAdmission;

/// A catalogue-owned selection, not a caller-supplied approval flag. Production
/// owners must establish exact activation/review/artifact provenance before
/// returning it. Draft behavioral validation uses a separate owner.
pub(crate) struct SelectedMontyRecipe {
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
    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure>;
    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure>;
}

struct Selection {
    selected: SelectedMontyRecipe,
    program_ref: String,
    execution: Mutex<Option<RetainedRecipeExecution>>,
}
#[derive(Default)]
struct State {
    task: Option<TaskHandle>,
    intent_started: bool,
    recipes: BTreeMap<Uuid, Arc<Selection>>,
}

/// Private supervisor-owned state retains failed executions and their original
/// child/Tool answers. Service settlement must precede releasing this object.
pub(crate) struct GlobalRecipePorts {
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    catalogue: Arc<dyn MontyTaskCatalogue>,
    user_input: String,
    ownership: crate::global_monty_owner::GlobalOwnerCheck,
    state: Mutex<State>,
    task_context: Mutex<Option<RetainedTaskContext>>,
    fenced: std::sync::atomic::AtomicBool,
}
impl GlobalRecipePorts {
    pub(crate) fn new(
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        catalogue: Arc<dyn MontyTaskCatalogue>,
        user_input: String,
        ownership: crate::global_monty_owner::GlobalOwnerCheck,
    ) -> Self {
        Self {
            host,
            admission,
            catalogue,
            user_input,
            ownership,
            state: Mutex::new(State::default()),
            task_context: Mutex::new(None),
            fenced: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn check_fence(&self) -> Result<(), PortFailure> {
        if self.fenced.load(std::sync::atomic::Ordering::Acquire) {
            return Err(failure("task_cancelled"));
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
        self.ownership
            .check()
            .await
            .map_err(|_| failure("monty_instance_ownership_failed"))?;
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
            "resolve_intent" => {
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
                }
                let selected = self.catalogue.resolve_intent(&self.user_input).await?;
                self.check_fence()?;
                match selected {
                    MontyIntentSelection::NoMatch => Ok(json!({"status":"no_match"})),
                    MontyIntentSelection::Disambiguation => Ok(json!({"status":"disambiguation"})),
                    MontyIntentSelection::Match(selected) => {
                        let (recipe_id, step_link) =
                            retain(&mut *self.state.lock().await, selected)?;
                        Ok(
                            json!({"status":"match", "component_id":recipe_id, "step_link":step_link}),
                        )
                    }
                }
            }
            "resolve_component_by_name" => {
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
                if self.state.lock().await.recipes.len() >= 8 {
                    return Err(failure("recipe_capacity_exceeded"));
                }
                let selected = self.catalogue.resolve_named_recipe(name).await?;
                self.check_fence()?;
                let (id, step_link) = retain(&mut *self.state.lock().await, selected)?;
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
                let inputs = program
                    .inputs()
                    .bind_task_inputs(selected.selected.inputs.as_ref().unwrap_or(envelope))
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
                let execution = RetainedRecipeExecution::new_child(
                    task,
                    parent,
                    selected.selected.inspected.clone(),
                )
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
            _ => {
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
fn retain(state: &mut State, selected: SelectedMontyRecipe) -> Result<(Uuid, String), PortFailure> {
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
            .bind_task_inputs(inputs)
            .map_err(|_| failure("recipe_composition_failed"))?;
    }
    if state.recipes.contains_key(&id) || state.recipes.len() >= 8 {
        return Err(failure("recipe_composition_failed"));
    }
    state.recipes.insert(
        id,
        Arc::new(Selection {
            selected,
            program_ref: Uuid::new_v4().to_string(),
            execution: Mutex::new(None),
        }),
    );
    Ok((id, link))
}
fn failure(reason: &str) -> PortFailure {
    PortFailure::new(reason).expect("static classified reason")
}
impl TaskPorts for GlobalRecipePorts {
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
            if let TaskOutcome::Completed { reply_ref } = &outcome {
                self.check_fence()?;
                let reference = serde_json::from_value(Value::String(reply_ref.clone()))
                    .map_err(|_| failure("recipe_reply_invalid"))?;
                self.host
                    .published_reply_content(&reference)
                    .map_err(|_| failure("recipe_reply_invalid"))?;
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
