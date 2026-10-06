//! Instance-root VM mechanics, isolated until the production upgrade gate.
//! The owning service supplies verified class-10 source, durable admitted work
//! and kernel ports. This type owns one VM, not a queue or Recipe workflow.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExecutionControl, ExecutionControlAction, ExecutionControlError,
    ExtFunctionResult, MontyObject, MontyUuid, PrintWriter, ResourceLimits, ResourceTracker,
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
    last_yield: Mutex<Duration>,
    slice: Duration,
}
impl ExecutionControl for RootControl {
    fn checkpoint(
        &self,
        elapsed: Duration,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        let mut last = self
            .last_yield
            .lock()
            .map_err(|_| ExecutionControlError::AccountingUnavailable)?;
        let delta = elapsed
            .checked_sub(*last)
            .ok_or(ExecutionControlError::AccountingUnavailable)?;
        if delta >= self.slice {
            *last = elapsed;
            Ok(ExecutionControlAction::Yield)
        } else {
            Ok(ExecutionControlAction::Continue)
        }
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
}

/// Exactly one owned global interpreter. No task budget is incorrectly charged
/// for the cumulative lifetime/shared coroutine clock of this root VM. The
/// production owner must account root work, enforce allocator/heap containment,
/// pair task tokens with exact fenced hosts and supervise the instance lock.
pub struct GlobalVm {
    progress: Option<Box<RunProgress>>,
    current: Option<ContinuationKey>,
    pending: BTreeMap<ContinuationKey, Pending>,
    aliases: BTreeSet<String>,
    receiver: MontyUuid,
    vm_id: Uuid,
    ordinal: u64,
    lifecycle: Lifecycle,
    bounds: GlobalBounds,
    stdout: String,
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
        tracker.set_execution_control(Arc::new(RootControl {
            last_yield: Mutex::new(Duration::ZERO),
            slice: bounds.values.execution_slice,
        }));
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
            aliases,
            receiver,
            vm_id,
            ordinal: 0,
            lifecycle: Lifecycle::Starting,
            bounds,
            stdout: String::new(),
        };
        let result = run.start(
            vec![host, MontyObject::int(i64::from(bounds.workers))],
            tracker,
            vm.print_writer(),
        );
        let boundary = vm.accept(result)?;
        Ok((vm, boundary))
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
        let Some(progress) = self.progress.take() else {
            return Err(VmError::kind(VmFailure::Terminal));
        };
        let RunProgress::ControlYield(paused) = *progress else {
            unreachable!("validated above")
        };
        self.current = None;
        let result = paused.resume(self.print_writer());
        self.accept(result)
    }

    /// The owning admission service validates exact message/run/attempt/history
    /// against durable state before calling this. Envelope shape is not authority.
    pub fn admit(&mut self, key: ContinuationKey, task: Value) -> Result<GlobalBoundary, VmError> {
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
        self.resolve_inner(key, HostAnswer::Return(task))
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
        self.resolve_inner(key, answer)
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
        self.resolve_inner(key, HostAnswer::Return(Value::Null))
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
    ) -> Result<GlobalBoundary, VmError> {
        let Some(pending) = self.pending.get(&key) else {
            let mut error = VmError::kind(VmFailure::ForeignContinuation);
            error.rejected_answer = Some(Box::new(answer));
            return Err(error);
        };
        let call_id = pending.call_id;
        if !matches!(self.progress.as_deref(), Some(RunProgress::ResolveFutures(waiting))
            if waiting.pending_call_ids().contains(&call_id))
        {
            let mut error = VmError::kind(VmFailure::WrongBoundary);
            error.rejected_answer = Some(Box::new(answer));
            return Err(error);
        }
        let converted = match &answer {
            HostAnswer::Return(value) => match json_input(value, self.bounds.values) {
                Ok(value) => ExtFunctionResult::Return(value),
                Err(mut error) => {
                    error.rejected_answer = Some(Box::new(answer));
                    // Admission has not consumed its work wait or started the
                    // task. Invalid/oversized user data rejects only that input.
                    if self.is_work_wait(key) && self.lifecycle == Lifecycle::Ready {
                        return Err(error);
                    }
                    return Err(self.fail(error));
                }
            },
            HostAnswer::Raise(error) | HostAnswer::Abort(error) => {
                ExtFunctionResult::Error(error.clone())
            }
        };
        let Some(progress) = self.progress.take() else {
            return Err(VmError::kind(VmFailure::Terminal));
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
        self.accept(result).map_err(|mut error| {
            error.rejected_answer = Some(Box::new(answer));
            error
        })
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
        result: Result<RunProgress, monty_types::MontyException>,
    ) -> Result<GlobalBoundary, VmError> {
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
                let (args, kwargs) = call_values(&call.args, self.bounds.values)
                    .map_err(|error| self.fail(error))?;
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
