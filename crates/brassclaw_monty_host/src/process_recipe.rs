//! Task-owned child contexts in the contained worker. No component lookup,
//! step selection, Tool execution, effect retry or durable acknowledgement.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

use brassclaw_resources::{
    LiveMontyTaskSettings, MontyTaskBudgetError, MontyTaskLimits, MontyTaskSettingsRevision,
    SharedMontyTaskBudget,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ContinuationKey, PythonArtifact, RecipeVm, VmBoundary, VmBounds, VmError, VmFailure,
    global::{GlobalBoundary, GlobalVm, Lifecycle},
    process::PortAnswer,
};

/// Private routing identity, not an attempt claim or a Tool grant. Issued only
/// at the real root admission boundary. Fatal admission retains the issued
/// handle and account as failure evidence; rejection before execution does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskHandle(Uuid);
impl std::fmt::Display for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecipeContextId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSettings {
    pub revision: u64,
    pub max_compute_time: Duration,
    pub token_budgets_enabled: bool,
}
impl From<TaskSettings> for MontyTaskSettingsRevision {
    fn from(settings: TaskSettings) -> Self {
        Self {
            revision: settings.revision,
            limits: MontyTaskLimits {
                max_compute_time: settings.max_compute_time,
                token_budgets_enabled: settings.token_budgets_enabled,
            },
        }
    }
}
impl From<MontyTaskSettingsRevision> for TaskSettings {
    fn from(settings: MontyTaskSettingsRevision) -> Self {
        Self {
            revision: settings.revision,
            max_compute_time: settings.limits.max_compute_time,
            token_budgets_enabled: settings.limits.token_budgets_enabled,
        }
    }
}

/// Supplied by the trusted manifest adapter. Integrity/syntax checking here is
/// not Q1/Q2, association approval, Tool policy or a pinned catalogue manifest.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedPython {
    pub source: String,
    pub checksum: [u8; 32],
    pub aliases: BTreeSet<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipeCommand {
    Open {
        task: TaskHandle,
        parent: Option<RecipeContextId>,
    },
    Start {
        context: RecipeContextId,
        selected: SelectedPython,
        inputs: Value,
    },
    ResumeHost {
        context: RecipeContextId,
        key: ContinuationKey,
        answer: PortAnswer,
    },
    ResumeControl {
        context: RecipeContextId,
        key: ContinuationKey,
    },
    CancelContext {
        context: RecipeContextId,
    },
    CancelTask {
        task: TaskHandle,
    },
    CloseTask {
        task: TaskHandle,
    },
    UpdateRuntimeSettings {
        expected_revision: u64,
        settings: TaskSettings,
        values: VmBounds,
    },
    UpdateSettings {
        expected_revision: u64,
        settings: TaskSettings,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipeBoundary {
    HostCall {
        key: ContinuationKey,
        name: String,
        args: Vec<Value>,
        kwargs: BTreeMap<String, Value>,
    },
    ControlYield {
        key: ContinuationKey,
    },
    Complete {
        value: Value,
    },
}
impl From<VmBoundary> for RecipeBoundary {
    fn from(boundary: VmBoundary) -> Self {
        match boundary {
            VmBoundary::HostCall(call) => Self::HostCall {
                key: call.continuation,
                name: call.name,
                args: call.args,
                kwargs: call.kwargs,
            },
            VmBoundary::ControlYield(key) => Self::ControlYield { key },
            VmBoundary::Complete(value) => Self::Complete { value },
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasedContext {
    pub context: RecipeContextId,
    pub pending_host: Option<ContinuationKey>,
    pub stdout: String,
}
/// Technical VM state only. Releasing a context never acknowledges external
/// effects, releases an attempt lease or authorizes task/step replay.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipeEvent {
    Opened {
        task: TaskHandle,
        context: RecipeContextId,
    },
    Progress {
        context: RecipeContextId,
        boundary: RecipeBoundary,
        stdout: String,
    },
    Released {
        task: Option<TaskHandle>,
        contexts: Vec<ReleasedContext>,
    },
    SettingsUpdated,
    CancellationRequested {
        task: TaskHandle,
    },
    Failed {
        context: Option<RecipeContextId>,
        stdout: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAccounting {
    pub task: TaskHandle,
    pub effective_revision: u64,
    pub compute_time: Option<Duration>,
    /// Explicit failure; None consumption must never be treated as zero.
    pub failure: Option<String>,
}

struct TaskRecord {
    run_id: String,
    budget: SharedMontyTaskBudget,
    contexts: BTreeSet<RecipeContextId>,
    root_opened: bool,
    cancel_requested: bool,
}
struct ContextRecord {
    task: TaskHandle,
    parent: Option<RecipeContextId>,
    vm: RecipeVm,
}
pub(crate) struct WorkerRecipes {
    tasks: BTreeMap<TaskHandle, TaskRecord>,
    contexts: BTreeMap<RecipeContextId, ContextRecord>,
    live: LiveMontyTaskSettings,
    max_tasks: usize,
    max_contexts: usize,
    bounds: VmBounds,
}
impl WorkerRecipes {
    pub(crate) fn bounds(&self) -> VmBounds {
        self.bounds
    }

    pub(crate) fn new(
        settings: TaskSettings,
        max_tasks: u32,
        max_contexts: u32,
        bounds: VmBounds,
    ) -> Result<Self, VmError> {
        if max_tasks == 0 || max_contexts < max_tasks || !bounds.valid() {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        let live = LiveMontyTaskSettings::new(settings.into())
            .map_err(|_| VmError::kind(VmFailure::InvalidBounds))?;
        Ok(Self {
            tasks: BTreeMap::new(),
            contexts: BTreeMap::new(),
            live,
            max_tasks: max_tasks as usize,
            max_contexts: max_contexts as usize,
            bounds,
        })
    }
    pub(crate) fn settings(&self) -> TaskSettings {
        self.live.current().into()
    }
    pub(crate) fn accounting(&self) -> Vec<TaskAccounting> {
        self.tasks
            .iter()
            .map(|(task, record)| match record.budget.check() {
                Ok(snapshot) => TaskAccounting {
                    task: *task,
                    effective_revision: snapshot.settings.revision,
                    compute_time: Some(snapshot.usage.compute_time),
                    failure: None,
                },
                Err(error) => TaskAccounting {
                    task: *task,
                    effective_revision: self.live.current().revision,
                    compute_time: None,
                    failure: Some(error.to_string()),
                },
            })
            .collect()
    }
    pub(crate) fn empty(&self) -> bool {
        self.tasks.is_empty() && self.contexts.is_empty()
    }

    pub(crate) fn admit(
        &mut self,
        root: &mut GlobalVm,
        key: ContinuationKey,
        mut input: Value,
        admitted_task: &mut Option<TaskHandle>,
    ) -> Result<GlobalBoundary, VmError> {
        if root.lifecycle() != Lifecycle::Ready {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        if self.tasks.len() >= self.max_tasks {
            return Err(VmError::kind(VmFailure::ResourceLimit));
        }
        // Protocol 3 accepts six data fields. The worker adds its own fresh
        // routing token; the parent never substitutes conversation/run IDs.
        let valid = input.as_object().is_some_and(|input| {
            input.len() == 6
                && ["conversation_id", "message_id", "turn_id", "run_id"]
                    .iter()
                    .all(|name| {
                        input
                            .get(*name)
                            .and_then(Value::as_str)
                            .is_some_and(|id| !id.is_empty())
                    })
                && input.get("user_input").is_some_and(Value::is_string)
                && input.get("history").is_some_and(Value::is_array)
        });
        if !valid {
            return Err(VmError::kind(VmFailure::InvalidInputs));
        }
        let run_id = input["run_id"]
            .as_str()
            .expect("validated above")
            .to_owned();
        if self.tasks.values().any(|task| task.run_id == run_id) {
            return Err(VmError::kind(VmFailure::InvalidInputs));
        }
        let task = TaskHandle(Uuid::new_v4());
        if self.tasks.contains_key(&task) {
            return Err(VmError::kind(VmFailure::Terminal));
        }
        input["task_token"] = Value::String(task.to_string());
        let budget = SharedMontyTaskBudget::new(self.live.clone());
        self.tasks.insert(
            task,
            TaskRecord {
                run_id,
                budget: budget.clone(),
                contexts: BTreeSet::new(),
                root_opened: false,
                cancel_requested: false,
            },
        );
        let result = root.admit(key, input, budget);
        if result.is_err() && root.lifecycle() != Lifecycle::Failed {
            // Input/continuation rejection did not start this task. No account
            // or run-id reservation may leak into the next valid admission.
            self.tasks.remove(&task);
        } else {
            // Publish the exact routing identity even if admitted Python failed
            // before its first port. Retain its account for fatal reconciliation.
            *admitted_task = Some(task);
        }
        result
    }

    pub(crate) fn apply(
        &mut self,
        command: RecipeCommand,
        root: &mut GlobalVm,
    ) -> Result<RecipeEvent, VmError> {
        let lifecycle = root.lifecycle();
        if !matches!(lifecycle, Lifecycle::Ready | Lifecycle::Stopping)
            && !matches!(
                &command,
                RecipeCommand::CancelContext { .. } | RecipeCommand::CloseTask { .. }
            )
        {
            return Err(VmError::kind(VmFailure::Terminal));
        }
        match command {
            RecipeCommand::Open { task, parent } => {
                if !matches!(lifecycle, Lifecycle::Ready | Lifecycle::Stopping)
                    || self.contexts.len() >= self.max_contexts
                {
                    return Err(VmError::kind(VmFailure::ResourceLimit));
                }
                let record = self
                    .tasks
                    .get_mut(&task)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
                if record.cancel_requested {
                    return Err(VmError::kind(VmFailure::Terminal));
                }
                if let Some(parent) = parent {
                    if self.contexts.get(&parent).is_none_or(|parent| {
                        parent.task != task || matches!(parent.vm.state, crate::VmState::Terminal)
                    }) {
                        return Err(VmError::kind(VmFailure::ForeignContinuation));
                    }
                } else if record.root_opened {
                    // Additional contexts require an explicit same-task parent.
                    return Err(VmError::kind(VmFailure::WrongBoundary));
                }
                let context = RecipeContextId(Uuid::new_v4());
                if self.contexts.contains_key(&context) {
                    return Err(VmError::kind(VmFailure::Terminal));
                }
                let vm = prepare(&record.budget, || {
                    RecipeVm::new(record.budget.clone(), self.bounds)
                })?;
                self.contexts
                    .insert(context, ContextRecord { task, parent, vm });
                record.contexts.insert(context);
                if parent.is_none() {
                    record.root_opened = true;
                }
                Ok(RecipeEvent::Opened { task, context })
            }
            RecipeCommand::Start {
                context,
                selected,
                inputs,
            } => {
                let owner = self
                    .contexts
                    .get_mut(&context)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
                if !matches!(owner.vm.state, crate::VmState::Idle(_)) {
                    return Err(VmError::kind(
                        if matches!(owner.vm.state, crate::VmState::Terminal) {
                            VmFailure::Terminal
                        } else {
                            VmFailure::WrongBoundary
                        },
                    ));
                }
                if inputs
                    .as_object()
                    .is_none_or(|inputs| inputs.keys().any(|key| !crate::input_identifier(key)))
                {
                    return Err(VmError::kind(VmFailure::InvalidInputs));
                }
                let task = self
                    .tasks
                    .get(&owner.task)
                    .ok_or_else(|| VmError::kind(VmFailure::Terminal))?;
                if task.cancel_requested {
                    return Err(VmError::kind(VmFailure::Terminal));
                }
                let artifact = prepare(&task.budget, || {
                    PythonArtifact::new(
                        Arc::from(selected.source),
                        selected.checksum,
                        selected.aliases,
                        self.bounds,
                    )
                })?;
                let boundary = owner.vm.start_step(Arc::new(artifact), inputs)?;
                Ok(RecipeEvent::Progress {
                    context,
                    boundary: boundary.into(),
                    stdout: owner.vm.take_stdout(),
                })
            }
            RecipeCommand::ResumeHost {
                context,
                key,
                answer,
            } => {
                let owner = self
                    .contexts
                    .get_mut(&context)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
                let boundary = owner
                    .vm
                    .resume_host(key, answer.into_host().map_err(VmError::kind)?)?;
                Ok(RecipeEvent::Progress {
                    context,
                    boundary: boundary.into(),
                    stdout: owner.vm.take_stdout(),
                })
            }
            RecipeCommand::ResumeControl { context, key } => {
                let owner = self
                    .contexts
                    .get_mut(&context)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
                let boundary = owner.vm.resume_control(key)?;
                Ok(RecipeEvent::Progress {
                    context,
                    boundary: boundary.into(),
                    stdout: owner.vm.take_stdout(),
                })
            }
            RecipeCommand::CancelContext { context } => {
                let task = self
                    .contexts
                    .get(&context)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?
                    .task;
                // Build the same-task descendant graph once. Cancellation of a
                // deep child chain must not repeatedly scan all live contexts.
                let mut children: BTreeMap<RecipeContextId, Vec<RecipeContextId>> = BTreeMap::new();
                for (id, record) in &self.contexts {
                    if record.task == task
                        && let Some(parent) = record.parent
                    {
                        children.entry(parent).or_default().push(*id);
                    }
                }
                let mut selected = BTreeSet::new();
                let mut pending = vec![context];
                while let Some(id) = pending.pop() {
                    if selected.insert(id)
                        && let Some(descendants) = children.remove(&id)
                    {
                        pending.extend(descendants);
                    }
                }
                let contexts = self.release(task, &selected)?;
                Ok(RecipeEvent::Released {
                    task: None,
                    contexts,
                })
            }
            RecipeCommand::CancelTask { task } => {
                let record = self
                    .tasks
                    .get_mut(&task)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
                root.request_task_cancellation(&task.to_string())?;
                record.cancel_requested = true;
                for context in &record.contexts {
                    self.contexts
                        .get(context)
                        .ok_or_else(|| VmError::kind(VmFailure::Terminal))?
                        .vm
                        .cancellation()
                        .request();
                }
                // Keep contexts, pending-call keys and accounting until the
                // owner reconciles them. Requested is not a completion ack.
                Ok(RecipeEvent::CancellationRequested { task })
            }
            RecipeCommand::CloseTask { task } => {
                if matches!(lifecycle, Lifecycle::Ready | Lifecycle::Stopping)
                    && root.task_active(&task.to_string())?
                {
                    // Child cancellation is not root cancellation or durable
                    // finish. Removing this record would reset task accounting
                    // while the same root coroutine could still execute.
                    return Err(VmError::kind(VmFailure::WrongBoundary));
                }
                let selected = self
                    .tasks
                    .get(&task)
                    .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?
                    .contexts
                    .clone();
                let contexts = self.release(task, &selected)?;
                self.tasks.remove(&task);
                Ok(RecipeEvent::Released {
                    task: Some(task),
                    contexts,
                })
            }
            RecipeCommand::UpdateRuntimeSettings {
                expected_revision,
                settings,
                values,
            } => {
                if !values.valid() {
                    return Err(VmError::kind(VmFailure::InvalidBounds));
                }
                // Publish first: failed validation/CAS leaves every VM unchanged.
                self.apply(
                    RecipeCommand::UpdateSettings {
                        expected_revision,
                        settings,
                    },
                    root,
                )?;
                root.update_bounds(values);
                for context in self.contexts.values_mut() {
                    context.vm.update_bounds(values);
                }
                self.bounds = values;
                Ok(RecipeEvent::SettingsUpdated)
            }
            RecipeCommand::UpdateSettings {
                expected_revision,
                settings,
            } => {
                self.live
                    .publish(expected_revision, settings.into())
                    .map_err(|error| {
                        VmError::kind(match error {
                            MontyTaskBudgetError::RevisionConflict => {
                                VmFailure::SettingsRevisionConflict
                            }
                            MontyTaskBudgetError::InvalidSettings => VmFailure::InvalidBounds,
                            _ => VmFailure::ResourceLimit,
                        })
                    })?;
                Ok(RecipeEvent::SettingsUpdated)
            }
        }
    }
    fn release(
        &mut self,
        task: TaskHandle,
        selected: &BTreeSet<RecipeContextId>,
    ) -> Result<Vec<ReleasedContext>, VmError> {
        let owner = self
            .tasks
            .get_mut(&task)
            .ok_or_else(|| VmError::kind(VmFailure::Terminal))?;
        if selected.iter().any(|id| {
            !owner.contexts.contains(id)
                || self
                    .contexts
                    .get(id)
                    .is_none_or(|context| context.task != task)
        }) {
            return Err(VmError::kind(VmFailure::Terminal));
        }
        let mut released = Vec::with_capacity(selected.len());
        for context in selected {
            let mut record = self.contexts.remove(context).expect("prevalidated context");
            owner.contexts.remove(context);
            let stopped = record.vm.cancel();
            released.push(ReleasedContext {
                context: *context,
                pending_host: stopped.pending_host,
                stdout: stopped.stdout,
            });
        }
        Ok(released)
    }
}

/// Constructor work has no VM execution clock or external wait. Charge it once
/// to the same task account, even on syntax/integrity failure. Child execution
/// and typed conversion use their own existing non-overlapping clock segments.
fn prepare<T>(
    budget: &SharedMontyTaskBudget,
    work: impl FnOnce() -> Result<T, VmError>,
) -> Result<T, VmError> {
    budget
        .check()
        .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
    let started = Instant::now();
    let result = work();
    budget
        .record_compute_time(started.elapsed())
        .and_then(|()| budget.check())
        .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
    result
}
