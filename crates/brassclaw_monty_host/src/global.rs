//! Instance-root VM mechanics, isolated until the production upgrade gate.
//! The owning service supplies verified class-10 source, durable admitted work
//! and kernel ports. This type owns one VM, not a queue or Recipe workflow.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use brassclaw_resources::{MontyTaskBudgetError, SharedMontyTaskBudget};
use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExecutionControl, ExecutionControlAction, ExecutionControlError,
    ExecutionObservation, ExtFunctionResult, MontyObject, MontyUuid, PrintWriter, ResourceLimits,
    ResourceTracker,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ContinuationKey, HOST_TYPE, HostAnswer, HostRequest, VmBounds, VmError, VmFailure, call_values,
    identifier, json_input,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Lifecycle {
    Starting,
    Ready,
    Stopping,
    Stopped,
    Failed,
}

/// This is a technical pending-call bound, not a token or task-duration budget.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalBounds {
    pub values: VmBounds,
    pub workers: u32,
    pub max_pending_calls: usize,
}

pub enum GlobalBoundary {
    HostCall(HostRequest),
    ControlYield(ContinuationKey),
    Waiting(Vec<ContinuationKey>),
    Stopped,
}

/// Actual host answers withheld from a failed task, for trusted reconciliation.
/// Neither taking this receipt nor releasing VM state settles an external effect.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithheldHostAnswer {
    pub continuation: ContinuationKey,
    pub answer: HostAnswer,
}
impl std::fmt::Debug for WithheldHostAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WithheldHostAnswer")
            .field("continuation", &self.continuation)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for GlobalBoundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostCall(call) => call.fmt(f),
            Self::ControlYield(key) => f.debug_tuple("ControlYield").field(key).finish(),
            Self::Waiting(keys) => f.debug_tuple("Waiting").field(keys).finish(),
            Self::Stopped => f.write_str("Stopped"),
        }
    }
}

#[derive(Debug)]
struct RootControl {
    account: Mutex<RootAccount>,
    slice: Duration,
    max_contexts: usize,
}
#[derive(Debug, Default)]
struct RootAccount {
    last_yield: Duration,
    execution: Duration,
    preparation: Duration,
    contexts: BTreeMap<Option<u32>, Duration>,
    preparation_contexts: BTreeMap<Option<u32>, Duration>,
    // Some(None) explicitly identifies service preparation. None leaves
    // interpreter export preparation with the currently loaded coroutine.
    preparation_owner: Option<Option<u32>>,
    adaptation: Duration,
    adaptation_contexts: BTreeMap<Option<u32>, Duration>,
    tasks: BTreeMap<u32, RootTaskAccount>,
}
#[derive(Debug)]
struct RootTaskAccount {
    token: String,
    budget: SharedMontyTaskBudget,
    failure: Option<MontyTaskBudgetError>,
    phase: TaskPhase,
    cancel_requested: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskPhase {
    Admitted,
    Running,
    Failing,
    Finishing,
}
impl RootControl {
    fn record_adaptation(&self, context: Option<u32>, duration: Duration) -> Result<(), VmError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        if !account.adaptation_contexts.contains_key(&context)
            && account.adaptation_contexts.len() >= self.max_contexts
        {
            return Err(VmError::kind(VmFailure::ResourceLimit));
        }
        account.adaptation = account
            .adaptation
            .checked_add(duration)
            .ok_or_else(|| VmError::kind(VmFailure::ResourceLimit))?;
        let consumed = account.adaptation_contexts.entry(context).or_default();
        *consumed = consumed
            .checked_add(duration)
            .ok_or_else(|| VmError::kind(VmFailure::ResourceLimit))?;
        if let Some(task) = context.and_then(|context| account.tasks.get_mut(&context))
            && let Err(error) = task
                .budget
                .record_compute_time(duration)
                .and_then(|()| task.budget.check().map(|_| ()))
        {
            task.failure.get_or_insert(error);
        }
        Ok(())
    }

    fn preparation_scope(
        self: &Arc<Self>,
        context: Option<u32>,
    ) -> Result<PreparationScope, VmError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        if account.preparation_owner.is_some() {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        account.preparation_owner = Some(context);
        Ok(PreparationScope {
            control: self.clone(),
            context,
            active: true,
        })
    }

    fn enter_task(&self, context: Option<u32>, token: &str) -> Result<(), VmError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        let task = context
            .and_then(|context| account.tasks.get_mut(&context))
            .filter(|task| task.token == token && task.phase == TaskPhase::Admitted)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidHostArguments))?;
        task.phase = TaskPhase::Running;
        Ok(())
    }

    fn interruption(
        &self,
        context: Option<u32>,
        pending: bool,
    ) -> Result<Option<&'static str>, VmError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        let Some(task) = context.and_then(|context| account.tasks.get_mut(&context)) else {
            return Ok(None);
        };
        if let Err(error) = task.budget.check() {
            task.failure.get_or_insert(error);
        }
        if task.phase != TaskPhase::Running || pending {
            return Ok(None);
        }
        if task.cancel_requested {
            task.phase = TaskPhase::Failing;
            Ok(Some("task_cancelled"))
        } else if let Some(error) = &task.failure {
            let reason = if *error == MontyTaskBudgetError::ComputeExceeded {
                "task_compute_exceeded"
            } else {
                "task_accounting_failed"
            };
            task.phase = TaskPhase::Failing;
            Ok(Some(reason))
        } else {
            Ok(None)
        }
    }

    fn validate_port(&self, context: Option<u32>, token: &str, name: &str) -> Result<(), VmError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        let task = context
            .and_then(|context| account.tasks.get_mut(&context))
            .filter(|task| task.token == token)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidHostArguments))?;
        if name == "finish_task" && matches!(task.phase, TaskPhase::Running | TaskPhase::Failing) {
            task.phase = TaskPhase::Finishing;
            return Ok(());
        }
        if task.phase != TaskPhase::Running {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        Ok(())
    }

    fn release_task(&self, context: u32) -> Result<(), VmError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        account.tasks.remove(&context);
        Ok(())
    }

    fn slice_action(
        &self,
        account: &mut RootAccount,
        elapsed: Duration,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        let delta = elapsed
            .checked_sub(account.last_yield)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        if delta >= self.slice {
            account.last_yield = elapsed;
            Ok(ExecutionControlAction::Yield)
        } else {
            Ok(ExecutionControlAction::Continue)
        }
    }
}

/// A serialized host resume owns import preparation, even when another
/// coroutine was the last to park. This does not change interpreter identity
/// or attribute bytecode execution to the host's selected continuation.
struct PreparationScope {
    control: Arc<RootControl>,
    context: Option<u32>,
    active: bool,
}
impl PreparationScope {
    fn finish(mut self) -> Result<(), VmError> {
        self.clear()
    }
    fn clear(&mut self) -> Result<(), VmError> {
        let mut account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        if account.preparation_owner != Some(self.context) {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        account.preparation_owner = None;
        self.active = false;
        Ok(())
    }
}
impl Drop for PreparationScope {
    fn drop(&mut self) {
        if self.active {
            // A failed clear leaves a poisoned/mismatched account, which the
            // next checkpoint rejects. Never fabricate a usable replacement.
            let _ = self.clear();
        }
    }
}

/// Cumulative interpreter telemetry. Contexts are scheduler coroutines, not
/// admitted tasks. Preparation is deliberately separate; a resumed host result
/// may belong to a coroutine other than the last one loaded in the VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootExecutionAccounting {
    pub execution: Duration,
    pub preparation: Duration,
    pub contexts: BTreeMap<Option<u32>, Duration>,
    pub preparation_contexts: BTreeMap<Option<u32>, Duration>,
    /// Host value conversion outside the interpreter's guarded windows.
    pub adaptation: Duration,
    pub adaptation_contexts: BTreeMap<Option<u32>, Duration>,
}
impl ExecutionControl for RootControl {
    fn checkpoint(
        &self,
        elapsed: Duration,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| ExecutionControlError::AccountingUnavailable)?;
        self.slice_action(&mut account, elapsed)
    }

    fn checkpoint_context(
        &self,
        observation: ExecutionObservation,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        let elapsed = observation
            .execution
            .checked_add(observation.preparation)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        let mut account = self
            .account
            .lock()
            .map_err(|_| ExecutionControlError::AccountingUnavailable)?;
        let delta = observation
            .execution
            .checked_sub(account.execution)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        let preparation_delta = observation
            .preparation
            .checked_sub(account.preparation)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        let preparation_context = account.preparation_owner.unwrap_or(observation.context);
        if (!account
            .preparation_contexts
            .contains_key(&preparation_context)
            && account.preparation_contexts.len() >= self.max_contexts)
            || (!account.contexts.contains_key(&observation.context)
                && account.contexts.len() >= self.max_contexts)
        {
            return Err(ExecutionControlError::AccountingUnavailable);
        }
        let compute = account.contexts.entry(observation.context).or_default();
        *compute = compute
            .checked_add(delta)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        let preparation = account
            .preparation_contexts
            .entry(preparation_context)
            .or_default();
        *preparation = preparation
            .checked_add(preparation_delta)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        account.execution = observation.execution;
        account.preparation = observation.preparation;
        if let Some(task) = observation
            .context
            .and_then(|context| account.tasks.get_mut(&context))
        {
            // Each interval belongs to the coroutine actually executing it.
            // The child registry holds a clone of this same task account.
            // A task failure must not become an uncatchable root control error.
            // At the next safe boundary the protected task handler receives it.
            if let Err(error) = task
                .budget
                .record_compute_time(delta)
                .and_then(|()| task.budget.check().map(|_| ()))
            {
                task.failure.get_or_insert(error);
            }
        }
        if let Some(task) = preparation_context.and_then(|context| account.tasks.get_mut(&context))
            && let Err(error) = task
                .budget
                .record_compute_time(preparation_delta)
                .and_then(|()| task.budget.check().map(|_| ()))
        {
            task.failure.get_or_insert(error);
        }
        self.slice_action(&mut account, elapsed)
    }
}
#[derive(Clone, Copy)]
enum CallKind {
    WorkWait(u32),
    Port,
}
struct Pending {
    call_id: u32,
    kind: CallKind,
    context: Option<u32>,
}

/// Actual interpreter generation and code/binding bytes. Passive observation,
/// not protected-root approval, an implementation artifact or Tool authority.
/// Only GlobalVm construction issues this identity; the private pipe transports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootExecutionIdentity {
    vm_id: Uuid,
    source_checksum: [u8; 32],
    aliases_checksum: [u8; 32],
    workers: u32,
}
impl RootExecutionIdentity {
    pub fn vm_id(&self) -> Uuid {
        self.vm_id
    }
    pub fn source_checksum(&self) -> [u8; 32] {
        self.source_checksum
    }
    pub fn aliases_checksum(&self) -> [u8; 32] {
        self.aliases_checksum
    }
    pub fn workers(&self) -> u32 {
        self.workers
    }
    /// Compare actual code/binding observations with an already retained
    /// definition. A match proves neither protected-root review nor activation.
    pub fn matches_definition(&self, source_checksum: [u8; 32], ports: &BTreeSet<String>) -> bool {
        self.source_checksum == source_checksum
            && self.aliases_checksum == root_aliases_checksum(ports)
    }
}
pub(crate) fn root_aliases_checksum(aliases: &BTreeSet<String>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"monty-root-port-bindings/1\0");
    digest.update((aliases.len() as u64).to_be_bytes());
    for alias in aliases {
        digest.update((alias.len() as u64).to_be_bytes());
        digest.update(alias.as_bytes());
    }
    digest.finalize().into()
}

/// Exactly one owned global interpreter. Tracked admission charges root execution
/// to the same task account as its child VMs; boot/service work remains separate.
/// Protected scopes support task-local interruption and correlated resume
/// preparation. The production owner must connect attempt cancellation and heap containment,
/// pair task tokens with exact fenced hosts and supervise the instance lock.
pub struct GlobalVm {
    progress: Option<Box<RunProgress>>,
    current: Option<ContinuationKey>,
    pending: BTreeMap<ContinuationKey, Pending>,
    aliases: BTreeSet<String>,
    receiver: MontyUuid,
    vm_id: Uuid,
    identity: RootExecutionIdentity,
    ordinal: u64,
    lifecycle: Lifecycle,
    bounds: GlobalBounds,
    stdout: String,
    control: Arc<RootControl>,
    withheld: BTreeMap<ContinuationKey, HostAnswer>,
}
impl GlobalVm {
    /// Perform the real work-wait handshake before returning a usable root.
    /// Startup control yields are resumed until all configured workers await
    /// work. The deadline covers compilation and execution, but synchronous
    /// compiler/native work still requires process containment for a bounded
    /// interruption guarantee; this method does not claim to preempt that work.
    pub fn start_ready(
        source: Arc<str>,
        checksum: [u8; 32],
        aliases: BTreeSet<String>,
        bounds: GlobalBounds,
        startup_timeout: Duration,
    ) -> Result<Self, VmError> {
        let started = Instant::now();
        let deadline = started
            .checked_add(startup_timeout)
            .filter(|_| !startup_timeout.is_zero())
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        let (mut vm, mut boundary) = Self::start(source, checksum, aliases, bounds)?;
        loop {
            // Check before publishing Ready, including if construction itself
            // consumed the deadline. A late handshake cannot admit work.
            if Instant::now() >= deadline {
                vm.abandon();
                let mut error = VmError::kind(VmFailure::StartupDeadline);
                error.stdout = vm.take_stdout();
                return Err(error);
            }
            boundary = match boundary {
                GlobalBoundary::HostCall(call) if call.name == "await_next_task" => {
                    vm.defer(call.continuation)?
                }
                GlobalBoundary::ControlYield(key) => vm.resume_control(key)?,
                GlobalBoundary::Waiting(_) if vm.lifecycle == Lifecycle::Ready => return Ok(vm),
                _ => {
                    vm.abandon();
                    let mut error = VmError::kind(VmFailure::WrongBoundary);
                    error.stdout = vm.take_stdout();
                    return Err(error);
                }
            };
        }
    }

    /// The caller obtains source/checksum from verified boot storage. Syntax and
    /// checksum checking here cannot establish bootstrap provenance or approval.
    pub fn start(
        source: Arc<str>,
        checksum: [u8; 32],
        aliases: BTreeSet<String>,
        bounds: GlobalBounds,
    ) -> Result<(Self, GlobalBoundary), VmError> {
        if !bounds.values.valid()
            || bounds.workers == 0
            || bounds.max_pending_calls < bounds.workers as usize
        {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        let max_contexts = (bounds.workers as usize)
            .checked_add(2)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        if source.len() > bounds.values.max_source_bytes {
            return Err(VmError::kind(VmFailure::SourceLimit));
        }
        if <[u8; 32]>::from(Sha256::digest(source.as_bytes())) != checksum {
            return Err(VmError::kind(VmFailure::Integrity));
        }
        if !aliases.contains("await_next_task") || aliases.iter().any(|name| !identifier(name)) {
            return Err(VmError::kind(VmFailure::InvalidBinding));
        }
        let run = MontyRun::new(
            source.to_string(),
            "global-orchestrator.py",
            vec!["host".into(), "worker_count".into()],
            CompileOptions::default(),
        )
        .map_err(|exception| VmError {
            failure: VmFailure::Python,
            exception: Some(Box::new(exception)),
            stdout: String::new(),
            rejected_answer: None,
        })?;
        let mut tracker = ResourceTracker::new(ResourceLimits::default());
        let control = Arc::new(RootControl {
            account: Mutex::new(RootAccount::default()),
            slice: bounds.values.execution_slice,
            // The verified root has a main coroutine, bounded workers, and
            // discarded-context service work. Unexpected coroutine growth
            // must fail closed instead of growing this lifetime map forever.
            max_contexts,
        });
        tracker.set_execution_control(control.clone());
        let vm_id = Uuid::new_v4();
        let receiver = MontyUuid::from_u128(vm_id.as_u128());
        let host = MontyObject::class_instance(
            MontyObject::class_type("BrassClawHost", HOST_TYPE, true, true, []),
            receiver,
            [],
        );
        let mut vm = Self {
            progress: None,
            current: None,
            pending: BTreeMap::new(),
            // The incoming catalogue metadata was decoded outside the VM
            // domain. Adopt the retained binding names inside hosting scope.
            aliases: aliases.clone(),
            receiver,
            vm_id,
            identity: RootExecutionIdentity {
                vm_id,
                source_checksum: checksum,
                aliases_checksum: root_aliases_checksum(&aliases),
                workers: bounds.workers,
            },
            ordinal: 0,
            lifecycle: Lifecycle::Starting,
            bounds,
            stdout: String::new(),
            control,
            withheld: BTreeMap::new(),
        };
        let result = run.start(
            vec![host, MontyObject::int(i64::from(bounds.workers))],
            tracker,
            vm.print_writer(),
        );
        let boundary = vm.accept(result)?;
        Ok((vm, boundary))
    }
    pub fn execution_identity(&self) -> RootExecutionIdentity {
        self.identity
    }

    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }
    /// Retained until the owner reconciles them, even after fatal VM failure.
    pub fn outstanding(&self) -> Vec<ContinuationKey> {
        self.pending.keys().copied().collect()
    }
    /// Current worker admission slots, never generic port-call continuations.
    /// Sorting by worker ID gives the owner a stable transport inventory without
    /// interpreting the private VM generation/ordinal embedded in each key.
    pub fn work_waits(&self) -> Vec<(u32, ContinuationKey)> {
        let mut waits: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(key, pending)| match pending.kind {
                CallKind::WorkWait(worker) => Some((worker, *key)),
                CallKind::Port => None,
            })
            .collect();
        waits.sort_unstable_by_key(|(worker, _)| *worker);
        waits
    }
    pub fn take_stdout(&mut self) -> String {
        std::mem::take(&mut self.stdout)
    }
    pub fn take_withheld_answers(&mut self) -> Vec<WithheldHostAnswer> {
        std::mem::take(&mut self.withheld)
            .into_iter()
            .map(|(continuation, answer)| WithheldHostAnswer {
                continuation,
                answer,
            })
            .collect()
    }

    /// Correlate an actual pending call with its interpreter context. This is
    /// routing telemetry only and grants no authority over an admitted task.
    pub fn pending_context(&self, key: ContinuationKey) -> Option<u32> {
        self.pending.get(&key).and_then(|pending| pending.context)
    }

    pub fn execution_accounting(&self) -> Result<RootExecutionAccounting, VmError> {
        let account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        Ok(RootExecutionAccounting {
            execution: account.execution,
            preparation: account.preparation,
            contexts: account.contexts.clone(),
            preparation_contexts: account.preparation_contexts.clone(),
            adaptation: account.adaptation,
            adaptation_contexts: account.adaptation_contexts.clone(),
        })
    }

    /// Convert the current actual async call to a pending future. This only
    /// releases the interpreter; it never executes the requested host operation.
    pub fn defer(&mut self, key: ContinuationKey) -> Result<GlobalBoundary, VmError> {
        if self.current != Some(key)
            || !matches!(self.progress.as_deref(), Some(RunProgress::FunctionCall(_)))
        {
            return Err(VmError::kind(VmFailure::ForeignContinuation));
        }
        let Some(progress) = self.progress.take() else {
            return Err(VmError::kind(VmFailure::Terminal));
        };
        let RunProgress::FunctionCall(call) = *progress else {
            unreachable!("validated above")
        };
        self.current = None;
        let result = call.resume_pending(self.print_writer());
        self.accept(result)
    }
    pub fn resume_control(&mut self, key: ContinuationKey) -> Result<GlobalBoundary, VmError> {
        if self.current != Some(key)
            || !matches!(self.progress.as_deref(), Some(RunProgress::ControlYield(_)))
        {
            return Err(VmError::kind(VmFailure::ForeignContinuation));
        }
        let context = match self.progress.as_deref() {
            Some(RunProgress::ControlYield(paused)) => paused.tracker().execution_context(),
            _ => unreachable!("validated above"),
        };
        let pending = self
            .pending
            .values()
            .any(|pending| pending.context == context);
        let interruption = self.control.interruption(context, pending);
        let interruption = interruption.map_err(|error| self.fail(error))?;
        let Some(progress) = self.progress.take() else {
            return Err(VmError::kind(VmFailure::Terminal));
        };
        let RunProgress::ControlYield(paused) = *progress else {
            unreachable!("validated above")
        };
        self.current = None;
        let result = if let Some(reason) = interruption {
            paused.raise(
                monty_types::MontyException::runtime_error(reason),
                self.print_writer(),
            )
        } else {
            paused.resume(self.print_writer())
        };
        self.accept(result)
    }

    /// The owning admission service validates exact message/run/attempt/history
    /// against durable state before calling this. Envelope shape is not authority.
    /// Associates the waiting coroutine with an already-created task account
    /// before any admitted Python runs. The token is routing data, not authority.
    /// Task-local interruption requires the trusted source's protected enter_task
    /// boundary. It never clears a terminal resource error or aborts another task.
    pub fn admit(
        &mut self,
        key: ContinuationKey,
        task: Value,
        budget: SharedMontyTaskBudget,
    ) -> Result<GlobalBoundary, VmError> {
        self.admit_inner(key, task, budget)
    }

    pub fn task_active(&self, token: &str) -> Result<bool, VmError> {
        let account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        Ok(account.tasks.values().any(|task| task.token == token))
    }

    /// Request interruption of one active scope. This does not settle its
    /// external calls or acknowledge task completion; its handler must finish.
    pub fn request_task_cancellation(&self, token: &str) -> Result<(), VmError> {
        if !matches!(self.lifecycle, Lifecycle::Ready | Lifecycle::Stopping) {
            return Err(VmError::kind(VmFailure::Terminal));
        }
        let mut account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        let task = account
            .tasks
            .values_mut()
            .find(|task| task.token == token)
            .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
        task.cancel_requested = true;
        Ok(())
    }

    /// Root accounting failures remain explicit, even when the trusted Python
    /// task handler needs to run to report the failure and release its state.
    pub fn task_compute_failure(
        &self,
        token: &str,
    ) -> Result<Option<MontyTaskBudgetError>, VmError> {
        let account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        Ok(account
            .tasks
            .values()
            .find(|task| task.token == token)
            .and_then(|task| task.failure.clone()))
    }

    fn admit_inner(
        &mut self,
        key: ContinuationKey,
        task: Value,
        budget: SharedMontyTaskBudget,
    ) -> Result<GlobalBoundary, VmError> {
        if self.lifecycle != Lifecycle::Ready || !self.is_work_wait(key) {
            let mut error = VmError::kind(VmFailure::WrongBoundary);
            error.rejected_answer = Some(Box::new(HostAnswer::Return(task)));
            return Err(error);
        }
        let valid = task.as_object().is_some_and(|task| {
            // Exact current root envelope. Rust-only claims/credentials and new
            // protocol fields cannot silently enter ordinary Python task state.
            task.len() == 7
                && [
                    "task_token",
                    "conversation_id",
                    "message_id",
                    "turn_id",
                    "run_id",
                ]
                .iter()
                .all(|name| {
                    task.get(*name)
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
                })
                && task.get("user_input").is_some_and(Value::is_string)
                && task.get("history").is_some_and(Value::is_array)
        });
        if !valid {
            let mut error = VmError::kind(VmFailure::InvalidInputs);
            error.rejected_answer = Some(Box::new(HostAnswer::Return(task)));
            return Err(error);
        }
        if budget.check().is_err() {
            let mut error = VmError::kind(VmFailure::ResourceLimit);
            error.rejected_answer = Some(Box::new(HostAnswer::Return(task)));
            return Err(error);
        }
        let binding = RootTaskAccount {
            token: task["task_token"]
                .as_str()
                .expect("validated above")
                .to_owned(),
            budget,
            failure: None,
            phase: TaskPhase::Admitted,
            cancel_requested: false,
        };
        self.resolve_inner(key, HostAnswer::Return(task), Some(binding))
    }
    /// Work-wait completion has separate admission/shutdown APIs. A generic
    /// callback returning null must never terminate a global worker. Task-local
    /// failures/cancellation use Raise so the root task coroutine handles them;
    /// Abort is reserved for fatal instance/runtime supervision in this API.
    pub fn resolve(
        &mut self,
        key: ContinuationKey,
        answer: HostAnswer,
    ) -> Result<GlobalBoundary, VmError> {
        if self.is_work_wait(key) {
            let mut error = VmError::kind(VmFailure::WrongBoundary);
            error.rejected_answer = Some(Box::new(answer));
            return Err(error);
        }
        self.resolve_inner(key, answer, None)
    }
    pub fn begin_shutdown(&mut self) -> Result<(), VmError> {
        if self.lifecycle != Lifecycle::Ready {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        self.lifecycle = Lifecycle::Stopping;
        Ok(())
    }
    /// Only an explicit instance shutdown sends None to a parked worker. This
    /// does not terminate busy tasks or falsely settle their external effects.
    pub fn close_worker(&mut self, key: ContinuationKey) -> Result<GlobalBoundary, VmError> {
        if self.lifecycle != Lifecycle::Stopping || !self.is_work_wait(key) {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        self.resolve_inner(key, HostAnswer::Return(Value::Null), None)
    }
    /// Fatal supervision releases the VM, retaining pending host correlation
    /// keys for reconciliation. Dropping it does not acknowledge external effects.
    pub fn abandon(&mut self) -> Vec<ContinuationKey> {
        self.progress = None;
        self.current = None;
        self.lifecycle = Lifecycle::Failed;
        self.outstanding()
    }
    fn is_work_wait(&self, key: ContinuationKey) -> bool {
        self.pending
            .get(&key)
            .is_some_and(|pending| matches!(pending.kind, CallKind::WorkWait(_)))
    }
    fn resolve_inner(
        &mut self,
        key: ContinuationKey,
        answer: HostAnswer,
        binding: Option<RootTaskAccount>,
    ) -> Result<GlobalBoundary, VmError> {
        let Some(pending) = self.pending.get(&key) else {
            let mut error = VmError::kind(VmFailure::ForeignContinuation);
            error.rejected_answer = Some(Box::new(answer));
            return Err(error);
        };
        let call_id = pending.call_id;
        let context = pending.context;
        if !matches!(self.progress.as_deref(), Some(RunProgress::ResolveFutures(waiting))
            if waiting.pending_call_ids().contains(&call_id))
        {
            let mut error = VmError::kind(VmFailure::WrongBoundary);
            error.rejected_answer = Some(Box::new(answer));
            return Err(error);
        }
        let conversion_started = Instant::now();
        let mut converted = match &answer {
            HostAnswer::Return(value) => match json_input(value, self.bounds.values) {
                Ok(value) => ExtFunctionResult::Return(value),
                Err(mut error) => {
                    error.rejected_answer = Some(Box::new(answer));
                    // Admission has not consumed its work wait or started the
                    // task. Invalid/oversized user data rejects only that input.
                    if self.is_work_wait(key) && self.lifecycle == Lifecycle::Ready {
                        // Rejected admission never owns a task account. Keep
                        // the actual conversion work as service telemetry.
                        if let Err(mut failure) = self
                            .control
                            .record_adaptation(None, conversion_started.elapsed())
                        {
                            failure.rejected_answer = error.rejected_answer.take();
                            return Err(self.fail(failure));
                        }
                        return Err(error);
                    }
                    if let Err(mut failure) = self
                        .control
                        .record_adaptation(context, conversion_started.elapsed())
                    {
                        failure.rejected_answer = error.rejected_answer.take();
                        return Err(self.fail(failure));
                    }
                    return Err(self.fail(error));
                }
            },
            HostAnswer::Raise(error) | HostAnswer::Abort(error) => {
                ExtFunctionResult::Error(error.clone())
            }
        };
        let conversion_time = conversion_started.elapsed();
        let work_wait = self.is_work_wait(key);
        if !work_wait
            && let Err(mut error) = self.control.record_adaptation(context, conversion_time)
        {
            error.rejected_answer = Some(Box::new(answer));
            return Err(self.fail(error));
        }
        let interruption = if !self.is_work_wait(key) && !matches!(&answer, HostAnswer::Abort(_)) {
            let pending = self
                .pending
                .iter()
                .any(|(other, pending)| *other != key && pending.context == context);
            let result = self.control.interruption(context, pending);
            match result {
                Ok(interruption) => interruption,
                Err(mut error) => {
                    error.rejected_answer = Some(Box::new(answer));
                    return Err(self.fail(error));
                }
            }
        } else {
            None
        };
        if let Some(reason) = interruption {
            if self.withheld.len() >= self.bounds.max_pending_calls {
                let mut error = VmError::kind(VmFailure::ResourceLimit);
                error.rejected_answer = Some(Box::new(answer));
                return Err(self.fail(error));
            }
            converted =
                ExtFunctionResult::Error(monty_types::MontyException::runtime_error(reason));
        }
        if let Some(binding) = binding {
            let Some(context) = context else {
                let mut error = VmError::kind(VmFailure::UnsupportedBoundary);
                error.rejected_answer = Some(Box::new(answer));
                return Err(self.fail(error));
            };
            let control = self.control.clone();
            let mut account = match control.account.lock() {
                Ok(account) => account,
                Err(_) => {
                    let mut error = VmError::kind(VmFailure::ResourceLimit);
                    error.rejected_answer = Some(Box::new(answer));
                    return Err(self.fail(error));
                }
            };
            if account.tasks.contains_key(&context)
                || account
                    .tasks
                    .values()
                    .any(|task| task.token == binding.token)
                || account.tasks.len() >= self.bounds.workers as usize
            {
                let mut error = VmError::kind(VmFailure::WrongBoundary);
                error.rejected_answer = Some(Box::new(answer));
                return Err(error);
            }
            // All shape, continuation and conversion checks preceded insertion.
            // From here the exact work wait will be consumed; even fatal Python
            // failure retains this binding for reconciliation, never for replay.
            account.tasks.insert(context, binding);
        }
        // Admission conversion is charged only after validation and binding.
        // Generic answers already belong to their exact active task context.
        if work_wait
            && let Err(mut error) = self.control.record_adaptation(context, conversion_time)
        {
            error.rejected_answer = Some(Box::new(answer));
            return Err(self.fail(error));
        }
        // ResolveFutures has no runnable workers. This command resolves one
        // exact pending future, so only its scope can become runnable. Import
        // preparation belongs to that scope, not the last parked coroutine.
        // Bytecode attribution remains the scheduler's observation.context.
        let scope = match self.control.preparation_scope(context) {
            Ok(scope) => scope,
            Err(mut error) => {
                error.rejected_answer = Some(Box::new(answer));
                return Err(self.fail(error));
            }
        };
        let Some(progress) = self.progress.take() else {
            let mut error = VmError::kind(VmFailure::Terminal);
            error.rejected_answer = Some(Box::new(answer));
            return Err(self.fail(error));
        };
        let RunProgress::ResolveFutures(waiting) = *progress else {
            unreachable!("validated above")
        };
        self.current = None;
        let result = match &answer {
            HostAnswer::Abort(error) => waiting.abort(error.clone(), self.print_writer()),
            _ => {
                self.pending.remove(&key);
                waiting.resume(vec![(call_id, converted)], self.print_writer())
            }
        };
        let accepted = self.accept(result);
        if let Err(mut error) = scope.finish() {
            error.rejected_answer = Some(Box::new(answer));
            return Err(self.fail(error));
        }
        if interruption.is_some() {
            self.withheld.insert(key, answer);
            accepted
        } else {
            accepted.map_err(|mut error| {
                error.rejected_answer = Some(Box::new(answer));
                error
            })
        }
    }
    fn print_writer(&mut self) -> PrintWriter<'_> {
        PrintWriter::CollectString(&mut self.stdout, Some(self.bounds.values.max_stdout_bytes))
    }
    fn fail(&mut self, mut error: VmError) -> VmError {
        self.progress = None;
        self.current = None;
        self.lifecycle = Lifecycle::Failed;
        error.stdout = self.take_stdout();
        error
    }
    fn key(&mut self) -> Result<ContinuationKey, VmError> {
        let Some(next) = self.ordinal.checked_add(1) else {
            return Err(self.fail(VmError::kind(VmFailure::Terminal)));
        };
        self.ordinal = next;
        Ok(ContinuationKey {
            vm_id: self.vm_id,
            ordinal: next,
        })
    }
    fn accept(
        &mut self,
        mut result: Result<RunProgress, monty_types::MontyException>,
    ) -> Result<GlobalBoundary, VmError> {
        let mut converted_call = None;
        let progress = loop {
            let progress = match result {
                Ok(progress) => progress,
                Err(exception) => {
                    return Err(self.fail(VmError {
                        failure: VmFailure::Python,
                        exception: Some(Box::new(exception)),
                        stdout: String::new(),
                        rejected_answer: None,
                    }));
                }
            };
            let RunProgress::FunctionCall(call) = &progress else {
                break progress;
            };
            if call.object_id != Some(self.receiver)
                || !self.aliases.contains(&call.function_name)
                || call.function_name == "await_next_task"
            {
                break progress;
            }
            let context = call.tracker().execution_context();
            let conversion_started = Instant::now();
            let values = call_values(&call.args, self.bounds.values);
            self.control
                .record_adaptation(context, conversion_started.elapsed())
                .map_err(|error| self.fail(error))?;
            let (args, kwargs) = values.map_err(|error| self.fail(error))?;
            let token = args
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| VmError::kind(VmFailure::InvalidHostArguments));
            let token = token.map_err(|error| self.fail(error))?;
            let local_answer = if call.function_name == "enter_task" {
                if args.len() != 1 || !kwargs.is_empty() || self.lifecycle != Lifecycle::Ready {
                    return Err(self.fail(VmError::kind(VmFailure::InvalidHostArguments)));
                }
                let entered = self.control.enter_task(context, token);
                entered.map_err(|error| self.fail(error))?;
                Some(Ok(MontyObject::none()))
            } else {
                let pending = self
                    .pending
                    .values()
                    .any(|pending| pending.context == context);
                if pending {
                    // The trusted root awaits each scope's operation before
                    // requesting another. Refuse invalid concurrent dispatch
                    // while retaining all actual pending calls for supervision.
                    return Err(self.fail(VmError::kind(VmFailure::UnsupportedBoundary)));
                }
                // Never inject while that task owns an unsettled external future.
                // Its real await/result path must settle the correlation first.
                let interruption = if call.function_name == "finish_task" {
                    Ok(None)
                } else {
                    self.control.interruption(context, pending)
                };
                let interruption = interruption.map_err(|error| self.fail(error))?;
                if let Some(reason) = interruption {
                    Some(Err(monty_types::MontyException::runtime_error(reason)))
                } else {
                    let valid = self
                        .control
                        .validate_port(context, token, &call.function_name);
                    valid.map_err(|error| self.fail(error))?;
                    None
                }
            };
            let Some(answer) = local_answer else {
                converted_call = Some((args, kwargs));
                break progress;
            };
            let RunProgress::FunctionCall(call) = progress else {
                unreachable!("validated above")
            };
            // This is a local hosting handshake or refusal before dispatch,
            // not a synthetic Tool result or a Rust-selected workflow step.
            // enter_task is synchronous hosting control. Refused task ports
            // fail at the call expression before an external future exists.
            // allow_eager_await is only a scheduling optimization; competing
            // runnable workers legitimately make that hint false.
            result = call.resume(
                answer.map_or_else(ExtFunctionResult::Error, ExtFunctionResult::Return),
                self.print_writer(),
            );
        };
        let boundary = match &progress {
            RunProgress::FunctionCall(call)
                if call.object_id == Some(self.receiver)
                    && self.aliases.contains(&call.function_name) =>
            {
                if self.pending.len() >= self.bounds.max_pending_calls
                    || self
                        .pending
                        .values()
                        .any(|pending| pending.call_id == call.call_id)
                {
                    return Err(self.fail(VmError::kind(VmFailure::UnsupportedBoundary)));
                }
                let (args, kwargs) = match converted_call.take() {
                    Some(values) => values,
                    None => {
                        let conversion_started = Instant::now();
                        let values = call_values(&call.args, self.bounds.values);
                        self.control
                            .record_adaptation(
                                call.tracker().execution_context(),
                                conversion_started.elapsed(),
                            )
                            .map_err(|error| self.fail(error))?;
                        values.map_err(|error| self.fail(error))?
                    }
                };
                let kind = if call.function_name == "await_next_task" {
                    let worker = if args.len() == 1 && kwargs.is_empty() {
                        args[0]
                            .as_u64()
                            .and_then(|worker| u32::try_from(worker).ok())
                    } else {
                        None
                    };
                    let Some(worker) = worker.filter(|worker| *worker < self.bounds.workers) else {
                        return Err(self.fail(VmError::kind(VmFailure::InvalidHostArguments)));
                    };
                    if self.pending.values().any(
                        |pending| matches!(pending.kind, CallKind::WorkWait(id) if id == worker),
                    ) {
                        return Err(self.fail(VmError::kind(VmFailure::UnsupportedBoundary)));
                    }
                    let context = call.tracker().execution_context();
                    // A trusted worker may return to admission only after its
                    // own pending host results have been settled. Preserve
                    // correlation and account ownership on malformed handoffs.
                    if self
                        .pending
                        .values()
                        .any(|pending| pending.context == context)
                    {
                        return Err(self.fail(VmError::kind(VmFailure::UnsupportedBoundary)));
                    }
                    if let Some(context) = context {
                        let released = self.control.release_task(context);
                        released.map_err(|error| self.fail(error))?;
                    }
                    CallKind::WorkWait(worker)
                } else {
                    if self.lifecycle == Lifecycle::Starting {
                        return Err(self.fail(VmError::kind(VmFailure::WrongBoundary)));
                    }
                    CallKind::Port
                };
                let key = self.key()?;
                self.pending.insert(
                    key,
                    Pending {
                        call_id: call.call_id,
                        kind,
                        context: call.tracker().execution_context(),
                    },
                );
                self.current = Some(key);
                GlobalBoundary::HostCall(HostRequest {
                    continuation: key,
                    name: call.function_name.clone(),
                    args,
                    kwargs,
                })
            }
            RunProgress::ResolveFutures(waiting) => {
                let ids: BTreeSet<_> = waiting.pending_call_ids().iter().copied().collect();
                let owned: BTreeSet<_> = self
                    .pending
                    .values()
                    .map(|pending| pending.call_id)
                    .collect();
                if ids.is_empty() || ids.len() != waiting.pending_call_ids().len() || ids != owned {
                    return Err(self.fail(VmError::kind(VmFailure::UnsupportedBoundary)));
                }
                if self.lifecycle == Lifecycle::Starting {
                    if self.pending.len() != self.bounds.workers as usize
                        || !self
                            .pending
                            .values()
                            .all(|pending| matches!(pending.kind, CallKind::WorkWait(_)))
                    {
                        return Err(self.fail(VmError::kind(VmFailure::WrongBoundary)));
                    }
                    self.lifecycle = Lifecycle::Ready;
                }
                self.current = None;
                GlobalBoundary::Waiting(self.outstanding())
            }
            RunProgress::ControlYield(_) => {
                let key = self.key()?;
                self.current = Some(key);
                GlobalBoundary::ControlYield(key)
            }
            RunProgress::Complete(value)
                if self.lifecycle == Lifecycle::Stopping
                    && self.pending.is_empty()
                    && value == &MontyObject::none() =>
            {
                self.lifecycle = Lifecycle::Stopped;
                self.current = None;
                return Ok(GlobalBoundary::Stopped);
            }
            RunProgress::FunctionCall(_) => {
                return Err(self.fail(VmError::kind(VmFailure::UnboundHostCall)));
            }
            _ => return Err(self.fail(VmError::kind(VmFailure::UnsupportedBoundary))),
        };
        self.progress = Some(Box::new(progress));
        Ok(boundary)
    }
}
