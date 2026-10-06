//! Mechanical child-VM hosting for Monty-owned Recipe execution.
//!
//! This is an isolated upgrade candidate, not the production global service.
//! Composition must supply integrity-checked, selected code and bindings after
//! its own catalogue/approval checks. The supervisor owns the exact admitted
//! task/attempt, kernel ports, external waits, effect reconciliation and resource
//! containment. This crate neither selects Recipes nor sequences their steps.
//! No database lookup, provider fallback, Tool dispatch or automatic retry lives
//! here. A continuation is an in-memory correlation key, never a Tool grant.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use brassclaw_resources::{MontyTaskBudgetError, MontyTaskClock, SharedMontyTaskBudget};
pub mod global;

use monty::{MontyRepl, MontyRun, ReplProgress, ReplStartError};
use monty_types::{
    CallArgs, CompileOptions, ExcType, ExecutionControl, ExecutionControlAction,
    ExecutionControlError, MontyException, MontyObject, MontyUuid, PrintWriter, ResourceLimits,
    ResourceTracker,
};
use num_traits::ToPrimitive;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const HOST_TYPE: MontyUuid = MontyUuid::from_u128(0x484f_5354_5459_5045);

/// Explicit technical bounds, separate from artificial token budgets.
#[derive(Debug, Clone, Copy)]
pub struct VmBounds {
    pub max_source_bytes: usize,
    pub max_compiled_source_bytes: usize,
    pub max_feeds: usize,
    pub max_stdout_bytes: usize,
    pub execution_slice: Duration,
    pub max_value_depth: usize,
    pub max_value_nodes: usize,
    pub max_value_bytes: usize,
}
impl VmBounds {
    fn valid(self) -> bool {
        self.max_source_bytes > 0
            && self.max_source_bytes <= self.max_compiled_source_bytes
            && self.max_feeds > 0
            && self.max_stdout_bytes > 0
            && self.execution_slice > Duration::ZERO
            && self.max_value_depth > 0
            && self.max_value_nodes > 0
            && self.max_value_bytes > 0
    }
}

/// One selected immutable source artifact and its callable aliases.
///
/// Integrity does not establish Q1/Q2, association approval, current Tool policy
/// or a coherent catalogue snapshot. Those remain the trusted caller's duties.
/// The same artifact handle stays attached through every suspension of its feed.
pub struct PythonArtifact {
    body: Arc<str>,
    checksum: [u8; 32],
    bindings: BTreeSet<String>,
}
impl fmt::Debug for PythonArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PythonArtifact")
            .field("checksum", &self.checksum)
            .field("bindings", &self.bindings)
            .finish_non_exhaustive()
    }
}
impl PythonArtifact {
    pub fn new(
        body: Arc<str>,
        expected_checksum: [u8; 32],
        bindings: BTreeSet<String>,
        bounds: VmBounds,
    ) -> Result<Self, VmError> {
        if !bounds.valid() {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        if body.len() > bounds.max_source_bytes {
            return Err(VmError::kind(VmFailure::SourceLimit));
        }
        if <[u8; 32]>::from(Sha256::digest(body.as_bytes())) != expected_checksum {
            return Err(VmError::kind(VmFailure::Integrity));
        }
        if bindings.iter().any(|name| !identifier(name)) {
            return Err(VmError::kind(VmFailure::InvalidBinding));
        }
        // Parse selected code before any effect. This is syntax evidence, not
        // approval or proof of meaning. Task data is supplied later as values.
        MontyRun::new(
            format!("{body}\nresult\n"),
            "component.py",
            vec!["host".into(), "inputs".into(), "result".into()],
            CompileOptions::default(),
        )
        .map_err(|exception| VmError {
            failure: VmFailure::Python,
            exception: Some(Box::new(exception)),
            stdout: String::new(),
            rejected_answer: None,
        })?;
        Ok(Self {
            body,
            checksum: expected_checksum,
            bindings,
        })
    }
}
fn identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic())
        && bytes.all(|b| b == b'_' || b.is_ascii_alphanumeric())
}

/// Bounded, exact in-memory continuation correlation; not authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContinuationKey {
    vm_id: Uuid,
    ordinal: u64,
}

/// Host arguments are private to the trusted port owner; Debug omits payloads.
pub struct HostRequest {
    pub continuation: ContinuationKey,
    pub name: String,
    pub args: Vec<serde_json::Value>,
    pub kwargs: BTreeMap<String, serde_json::Value>,
}
impl fmt::Debug for HostRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostRequest")
            .field("continuation", &self.continuation)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
/// Exactly one mechanical boundary. The global Python caller decides what next.
pub enum VmBoundary {
    HostCall(HostRequest),
    ControlYield(ContinuationKey),
    Complete(serde_json::Value),
}
impl fmt::Debug for VmBoundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostCall(call) => call.fmt(f),
            Self::ControlYield(key) => f.debug_tuple("ControlYield").field(key).finish(),
            Self::Complete(_) => f.write_str("Complete(<private result>)"),
        }
    }
}

/// The trusted port classifies catchable domain failures separately from
/// terminal cancellation, denial and unresolved effects. No implicit retry.
pub enum HostAnswer {
    Return(serde_json::Value),
    Raise(MontyException),
    Abort(MontyException),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmFailure {
    InvalidBounds,
    InvalidInputs,
    Integrity,
    InvalidBinding,
    SourceLimit,
    WrongBoundary,
    ForeignContinuation,
    Terminal,
    MissingResult,
    UnboundHostCall,
    UnsupportedBoundary,
    InvalidResult,
    InvalidHostArguments,
    ResourceLimit,
    Python,
}
/// Full diagnostic/output evidence stays available to the trusted supervisor.
/// Formatting deliberately includes neither Python exception text nor stdout.
pub struct VmError {
    pub failure: VmFailure,
    pub exception: Option<Box<MontyException>>,
    pub stdout: String,
    /// A rejected late/wrongly addressed reply remains evidence for the trusted
    /// supervisor. Formatting never prints its payload or exception text.
    pub rejected_answer: Option<Box<HostAnswer>>,
}
impl VmError {
    fn kind(failure: VmFailure) -> Self {
        Self {
            failure,
            exception: None,
            stdout: String::new(),
            rejected_answer: None,
        }
    }
}
impl fmt::Debug for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VmError")
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Monty child VM failed: {:?}", self.failure)
    }
}
impl std::error::Error for VmError {}

#[derive(Debug)]
struct TaskControl {
    budget: SharedMontyTaskBudget,
    clock: Mutex<MontyTaskClock>,
    cancelled: AtomicBool,
    last_yield: Mutex<Duration>,
    slice: Duration,
}
impl ExecutionControl for TaskControl {
    fn checkpoint(
        &self,
        elapsed: Duration,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        self.clock
            .lock()
            .map_err(|_| ExecutionControlError::AccountingUnavailable)?
            .checkpoint(elapsed)
            .map_err(|error| match error {
                MontyTaskBudgetError::ComputeExceeded => ExecutionControlError::TaskComputeExceeded,
                _ => ExecutionControlError::AccountingUnavailable,
            })?;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(ExecutionControlError::Cancelled);
        }
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
/// Trusted supervisor signal. VM acknowledgement and external call settlement
/// are separate; setting this flag does not prove either has completed.
#[derive(Clone)]
pub struct VmCancellation {
    control: Arc<TaskControl>,
}
impl VmCancellation {
    pub fn request(&self) {
        self.control.cancelled.store(true, Ordering::Release);
    }
}

enum VmState {
    Idle(Box<MontyRepl>),
    Paused {
        key: ContinuationKey,
        progress: Box<ReplProgress>,
    },
    Terminal,
}

/// One task-owned Recipe context. No unrelated task or replacement attempt may
/// reuse it implicitly. The global Monty service retains it while a host waits.
pub struct RecipeVm {
    state: VmState,
    artifact: Option<Arc<PythonArtifact>>,
    control: Arc<TaskControl>,
    receiver: MontyUuid,
    vm_id: Uuid,
    ordinal: u64,
    feeds: usize,
    compiled_bytes: usize,
    bounds: VmBounds,
    stdout: String,
}
impl RecipeVm {
    pub fn new(budget: SharedMontyTaskBudget, bounds: VmBounds) -> Result<Self, VmError> {
        if !bounds.valid() {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        let control = Arc::new(TaskControl {
            clock: Mutex::new(budget.execution_clock(Duration::ZERO)),
            budget,
            cancelled: AtomicBool::new(false),
            last_yield: Mutex::new(Duration::ZERO),
            slice: bounds.execution_slice,
        });
        let mut tracker = ResourceTracker::new(ResourceLimits::default());
        tracker.set_execution_control(control.clone());
        let vm_id = Uuid::new_v4();
        Ok(Self {
            state: VmState::Idle(Box::new(MontyRepl::new(
                "recipe.py",
                tracker,
                CompileOptions::default(),
            ))),
            artifact: None,
            control,
            receiver: MontyUuid::from_u128(vm_id.as_u128()),
            vm_id,
            ordinal: 0,
            feeds: 0,
            compiled_bytes: 0,
            bounds,
            stdout: String::new(),
        })
    }
    pub fn cancellation(&self) -> VmCancellation {
        VmCancellation {
            control: self.control.clone(),
        }
    }

    /// Return all produced stdout for persistence before draining this segment.
    /// Capacity overflow raises a VM error; it never silently truncates output.
    pub fn take_stdout(&mut self) -> String {
        std::mem::take(&mut self.stdout)
    }

    /// Called for exactly the step selected by Monty, not an internal Rust loop.
    /// Inputs are bound as a mapping. Existing Recipe locals survive; `result`
    /// is reset so a missing assignment cannot reuse the prior step's result.
    pub fn start_step(
        &mut self,
        artifact: Arc<PythonArtifact>,
        inputs: serde_json::Value,
    ) -> Result<VmBoundary, VmError> {
        if !matches!(self.state, VmState::Idle(_)) {
            return Err(VmError::kind(if matches!(self.state, VmState::Terminal) {
                VmFailure::Terminal
            } else {
                VmFailure::WrongBoundary
            }));
        }
        let inputs = self.adapt(|bounds| {
            let Some(mapping) = inputs.as_object() else {
                return Err(VmError::kind(VmFailure::InvalidInputs));
            };
            if mapping.keys().any(|name| !input_identifier(name)) {
                return Err(VmError::kind(VmFailure::InvalidInputs));
            }
            json_input(&inputs, bounds)
        })?;
        // A constant result expression supports the class-22 assignment contract.
        // No task values or source interpolation are involved.
        let bytes = artifact.body.len().checked_add("\nresult\n".len());
        let total = bytes.and_then(|bytes| self.compiled_bytes.checked_add(bytes));
        if artifact.body.len() > self.bounds.max_source_bytes
            || self.feeds >= self.bounds.max_feeds
            || total.is_none_or(|total| total > self.bounds.max_compiled_source_bytes)
        {
            return Err(VmError::kind(VmFailure::SourceLimit));
        }
        let VmState::Idle(repl) = std::mem::replace(&mut self.state, VmState::Terminal) else {
            unreachable!()
        };
        self.compiled_bytes = total.expect("checked above");
        self.feeds += 1;
        let code = format!("{}\nresult\n", artifact.body);
        self.artifact = Some(artifact);
        let host = MontyObject::class_instance(
            MontyObject::class_type("BrassClawHost", HOST_TYPE, true, true, []),
            self.receiver,
            [],
        );
        let result = repl.feed_start(
            &code,
            vec![
                ("host".into(), host),
                ("inputs".into(), inputs),
                ("result".into(), MontyObject::ellipsis()),
            ],
            PrintWriter::CollectString(&mut self.stdout, Some(self.bounds.max_stdout_bytes)),
        );
        self.accept(result)
    }

    pub fn resume_host(
        &mut self,
        key: ContinuationKey,
        answer: HostAnswer,
    ) -> Result<VmBoundary, VmError> {
        if let Err(mut error) = self.validate_key(key, false) {
            error.rejected_answer = Some(Box::new(answer));
            return Err(error);
        }
        let VmState::Paused { progress, .. } =
            std::mem::replace(&mut self.state, VmState::Terminal)
        else {
            unreachable!()
        };
        let ReplProgress::FunctionCall(call) = *progress else {
            unreachable!()
        };
        if self.control.cancelled.load(Ordering::Acquire) {
            let result = call.abort(cancel_exception(), self.print_writer());
            return self.accept(result).map_err(|mut error| {
                error.rejected_answer = Some(Box::new(answer));
                error
            });
        }
        let result = match answer {
            HostAnswer::Abort(error) => call.abort(error, self.print_writer()),
            HostAnswer::Return(value) => {
                let converted = match self.adapt(|bounds| json_input(&value, bounds)) {
                    Ok(converted) => converted,
                    Err(mut error) => {
                        self.artifact = None;
                        error.stdout.push_str(&self.take_stdout());
                        error.rejected_answer = Some(Box::new(HostAnswer::Return(value)));
                        return Err(error);
                    }
                };
                let result = if call.allow_eager_await {
                    call.resume_eager(Ok(converted), self.print_writer())
                } else {
                    call.resume(converted, self.print_writer())
                };
                // A completed external return remains evidence even if the next
                // opcode, live budget check or result validation fails. Never
                // turn that failure into permission to repeat the effect.
                return self.accept(result).map_err(|mut error| {
                    error.rejected_answer = Some(Box::new(HostAnswer::Return(value)));
                    error
                });
            }
            HostAnswer::Raise(error) if call.allow_eager_await => {
                call.resume_eager(Err(error), self.print_writer())
            }
            HostAnswer::Raise(error) => call.resume(
                monty_types::ExtFunctionResult::Error(error),
                self.print_writer(),
            ),
        };
        self.accept(result)
    }

    /// Release this task's VM without waiting for an external operation. The
    /// returned pending key still requires effect/fence reconciliation; this
    /// acknowledgement says nothing about provider/process quiescence.
    pub fn cancel(&mut self) -> VmStopped {
        self.control.cancelled.store(true, Ordering::Release);
        let state = std::mem::replace(&mut self.state, VmState::Terminal);
        let pending_host = match &state {
            VmState::Paused { key, progress }
                if matches!(progress.as_ref(), ReplProgress::FunctionCall(_)) =>
            {
                Some(*key)
            }
            _ => None,
        };
        drop(state);
        self.artifact = None;
        VmStopped {
            pending_host,
            stdout: self.take_stdout(),
        }
    }

    pub fn resume_control(&mut self, key: ContinuationKey) -> Result<VmBoundary, VmError> {
        self.validate_key(key, true)?;
        let VmState::Paused { progress, .. } =
            std::mem::replace(&mut self.state, VmState::Terminal)
        else {
            unreachable!()
        };
        let ReplProgress::ControlYield(state) = *progress else {
            unreachable!()
        };
        let result = state.resume(PrintWriter::CollectString(
            &mut self.stdout,
            Some(self.bounds.max_stdout_bytes),
        ));
        self.accept(result)
    }

    fn print_writer(&mut self) -> PrintWriter<'_> {
        PrintWriter::CollectString(&mut self.stdout, Some(self.bounds.max_stdout_bytes))
    }

    /// Charge only this synchronous data adapter. Never wrap interpreter calls,
    /// provider waits or a child in this wall-clock interval: those own separate
    /// clocks and would otherwise be charged twice or include external waits.
    fn adapt<T>(
        &mut self,
        operation: impl FnOnce(VmBounds) -> Result<T, VmError>,
    ) -> Result<T, VmError> {
        let check = || {
            if self.control.cancelled.load(Ordering::Acquire) {
                let mut failure = VmError::kind(VmFailure::ResourceLimit);
                failure.exception = Some(Box::new(cancel_exception()));
                return Err(failure);
            }
            self.control.budget.check().map(|_| ()).map_err(|error| {
                let control_error = match error {
                    MontyTaskBudgetError::ComputeExceeded => {
                        ExecutionControlError::TaskComputeExceeded
                    }
                    _ => ExecutionControlError::AccountingUnavailable,
                };
                let mut failure = VmError::kind(VmFailure::ResourceLimit);
                failure.exception = Some(Box::new(MontyException::new(
                    ExcType::RuntimeError,
                    Some(format!("{control_error:?}")),
                )));
                failure
            })
        };
        let result = check().and_then(|()| {
            let started = Instant::now();
            let result = operation(self.bounds);
            let charge = self.control.budget.record_compute_time(started.elapsed());
            // Check even a rejected value, and never expose successful output
            // or a dispatch request if its conversion exhausted the account.
            check()?;
            charge.map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
            result
        });
        if let Err(error) = &result
            && error.failure == VmFailure::ResourceLimit
        {
            self.state = VmState::Terminal;
            self.artifact = None;
        }
        result.map_err(|mut error| {
            if error.failure == VmFailure::ResourceLimit {
                error.stdout = self.take_stdout();
            }
            error
        })
    }

    fn validate_key(&self, key: ContinuationKey, control: bool) -> Result<(), VmError> {
        match &self.state {
            VmState::Paused {
                key: expected,
                progress,
            } if *expected == key => {
                if matches!(progress.as_ref(), ReplProgress::ControlYield(_)) == control {
                    Ok(())
                } else {
                    Err(VmError::kind(VmFailure::WrongBoundary))
                }
            }
            VmState::Terminal => Err(VmError::kind(VmFailure::Terminal)),
            _ => Err(VmError::kind(VmFailure::ForeignContinuation)),
        }
    }

    fn accept(
        &mut self,
        result: Result<ReplProgress, Box<ReplStartError>>,
    ) -> Result<VmBoundary, VmError> {
        let progress = match result {
            Ok(progress) => progress,
            Err(error) => {
                self.artifact = None;
                self.state = VmState::Terminal;
                return Err(VmError {
                    failure: VmFailure::Python,
                    exception: Some(Box::new(error.error)),
                    stdout: self.take_stdout(),
                    rejected_answer: None,
                });
            }
        };
        match progress {
            ReplProgress::Complete { repl, value } => {
                self.artifact = None;
                if value == MontyObject::ellipsis() {
                    self.state = VmState::Terminal;
                    return Err(VmError {
                        failure: VmFailure::MissingResult,
                        exception: None,
                        stdout: self.take_stdout(),
                        rejected_answer: None,
                    });
                }
                let value = match self.adapt(|bounds| json_output(value.as_ref(), bounds)) {
                    Ok(value) => value,
                    Err(mut error) => {
                        self.state = VmState::Terminal;
                        error.stdout.push_str(&self.take_stdout());
                        return Err(error);
                    }
                };
                self.state = VmState::Idle(Box::new(repl));
                Ok(VmBoundary::Complete(value))
            }
            ReplProgress::FunctionCall(call)
                if call.object_id == Some(self.receiver)
                    && self.artifact.as_ref().is_some_and(|artifact| {
                        artifact.bindings.contains(&call.function_name)
                    }) =>
            {
                let arguments = self.adapt(|bounds| call_values(&call.args, bounds));
                let (args, kwargs) = match arguments {
                    Ok(arguments) => arguments,
                    Err(mut error) => {
                        self.artifact = None;
                        self.state = VmState::Terminal;
                        error.stdout.push_str(&self.take_stdout());
                        return Err(error);
                    }
                };
                let key = self.next_key()?;
                let request = HostRequest {
                    continuation: key,
                    name: call.function_name.clone(),
                    args,
                    kwargs,
                };
                self.state = VmState::Paused {
                    key,
                    progress: Box::new(ReplProgress::FunctionCall(call)),
                };
                Ok(VmBoundary::HostCall(request))
            }
            ReplProgress::ControlYield(state) => {
                let key = self.next_key()?;
                self.state = VmState::Paused {
                    key,
                    progress: Box::new(ReplProgress::ControlYield(state)),
                };
                Ok(VmBoundary::ControlYield(key))
            }
            progress => {
                // No automatic lookup, OS dispatch, unsupported future resolution
                // or unbound Tool call. Abandon this task VM, never replay it.
                let failure = if matches!(progress, ReplProgress::FunctionCall(_)) {
                    VmFailure::UnboundHostCall
                } else {
                    VmFailure::UnsupportedBoundary
                };
                drop(progress);
                self.artifact = None;
                self.state = VmState::Terminal;
                Err(VmError {
                    failure,
                    exception: None,
                    stdout: self.take_stdout(),
                    rejected_answer: None,
                })
            }
        }
    }
    fn next_key(&mut self) -> Result<ContinuationKey, VmError> {
        let Some(next) = self.ordinal.checked_add(1) else {
            self.artifact = None;
            self.state = VmState::Terminal;
            return Err(VmError::kind(VmFailure::Terminal));
        };
        self.ordinal = next;
        Ok(ContinuationKey {
            vm_id: self.vm_id,
            ordinal: self.ordinal,
        })
    }
}
fn cancel_exception() -> MontyException {
    MontyException::new(ExcType::RuntimeError, Some("task_cancelled".into()))
}

/// VM-local cancellation acknowledgement, not external-call completion.
pub struct VmStopped {
    pub pending_host: Option<ContinuationKey>,
    pub stdout: String,
}
fn input_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b == b'_' || b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Transport bounds do not replace the selected component's recursive schema,
/// required/default rules or Tool validation. They apply across a whole call,
/// including all positional/keyword values, rather than separately per value.
struct ValueBudget {
    bounds: VmBounds,
    nodes: usize,
    bytes: usize,
    failure: VmFailure,
}
impl ValueBudget {
    fn new(bounds: VmBounds, failure: VmFailure) -> Self {
        Self {
            bounds,
            nodes: 0,
            bytes: 0,
            failure,
        }
    }
    fn node(&mut self, depth: usize) -> Result<(), VmError> {
        if depth > self.bounds.max_value_depth || self.nodes >= self.bounds.max_value_nodes {
            return Err(VmError::kind(self.failure));
        }
        self.nodes += 1;
        Ok(())
    }
    fn bytes(&mut self, bytes: usize) -> Result<(), VmError> {
        let Some(total) = self
            .bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.bounds.max_value_bytes)
        else {
            return Err(VmError::kind(self.failure));
        };
        self.bytes = total;
        Ok(())
    }
}
fn json_input(value: &serde_json::Value, bounds: VmBounds) -> Result<MontyObject, VmError> {
    fn convert(
        value: &serde_json::Value,
        depth: usize,
        budget: &mut ValueBudget,
    ) -> Result<MontyObject, VmError> {
        budget.node(depth)?;
        Ok(match value {
            serde_json::Value::Null => MontyObject::none(),
            serde_json::Value::Bool(value) => MontyObject::bool(*value),
            serde_json::Value::String(value) => {
                budget.bytes(value.len())?;
                MontyObject::string(value)
            }
            serde_json::Value::Number(value) => {
                if let Some(value) = value.as_i64() {
                    MontyObject::int(value)
                } else if let Some(value) = value.as_u64() {
                    MontyObject::bigint(value.into())
                } else {
                    MontyObject::float(
                        value
                            .as_f64()
                            .ok_or_else(|| VmError::kind(budget.failure))?,
                    )
                }
            }
            serde_json::Value::Array(values) => MontyObject::list(
                values
                    .iter()
                    .map(|value| convert(value, depth + 1, budget))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            serde_json::Value::Object(values) => MontyObject::dict(
                values
                    .iter()
                    .map(|(key, value)| {
                        budget.bytes(key.len())?;
                        Ok((MontyObject::string(key), convert(value, depth + 1, budget)?))
                    })
                    .collect::<Result<Vec<_>, VmError>>()?,
            ),
        })
    }
    convert(
        value,
        0,
        &mut ValueBudget::new(bounds, VmFailure::InvalidInputs),
    )
}
fn json_output(
    value: monty_types::ObjectRef<'_>,
    bounds: VmBounds,
) -> Result<serde_json::Value, VmError> {
    output_value(
        value,
        0,
        &mut ValueBudget::new(bounds, VmFailure::InvalidResult),
    )
}
fn output_value(
    value: monty_types::ObjectRef<'_>,
    depth: usize,
    budget: &mut ValueBudget,
) -> Result<serde_json::Value, VmError> {
    // The version-pinned transport adapter borrows the graph. Stable items()/pairs()
    // allocate entire containers before traversal; repr() can format enormous
    // BigInts. Neither belongs in a bounded data adapter.
    use monty_types::unstable::{MontyNode, child, node};
    budget.node(depth)?;
    Ok(match node(value) {
        MontyNode::None => serde_json::Value::Null,
        MontyNode::Bool(value) => serde_json::Value::Bool(*value),
        MontyNode::String(value) => {
            budget.bytes(value.len())?;
            serde_json::Value::String(value.clone())
        }
        MontyNode::Int(value) => serde_json::Value::from(*value),
        MontyNode::BigInt(value) => {
            if let Some(value) = value.to_i64() {
                serde_json::Value::from(value)
            } else {
                serde_json::Value::from(
                    value
                        .to_u64()
                        .ok_or_else(|| VmError::kind(budget.failure))?,
                )
            }
        }
        MontyNode::Float(value) => serde_json::Value::Number(
            serde_json::Number::from_f64(*value).ok_or_else(|| VmError::kind(budget.failure))?,
        ),
        MontyNode::List(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|id| output_value(child(value, *id), depth + 1, budget))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        MontyNode::Dict(values) => {
            let mut object = serde_json::Map::new();
            for (key, item) in values {
                let key = child(value, *key);
                let key = key.as_str().ok_or_else(|| VmError::kind(budget.failure))?;
                budget.bytes(key.len())?;
                if object.contains_key(key) {
                    return Err(VmError::kind(budget.failure));
                }
                object.insert(
                    key.into(),
                    output_value(child(value, *item), depth + 1, budget)?,
                );
            }
            serde_json::Value::Object(object)
        }
        _ => return Err(VmError::kind(budget.failure)),
    })
}

fn call_values(
    args: &CallArgs,
    bounds: VmBounds,
) -> Result<(Vec<serde_json::Value>, BTreeMap<String, serde_json::Value>), VmError> {
    let mut transport = ValueBudget::new(bounds, VmFailure::InvalidHostArguments);
    let positional = args
        .args()
        .map(|value| output_value(value, 0, &mut transport))
        .collect::<Result<Vec<_>, _>>()?;
    let mut kwargs = BTreeMap::new();
    for (key, value) in args.kwargs() {
        let key = key
            .as_str()
            .filter(|name| identifier(name))
            .ok_or_else(|| VmError::kind(VmFailure::InvalidHostArguments))?;
        transport.bytes(key.len())?;
        if kwargs.contains_key(key) {
            return Err(VmError::kind(VmFailure::InvalidHostArguments));
        }
        kwargs.insert(key.to_owned(), output_value(value, 0, &mut transport)?);
    }
    Ok((positional, kwargs))
}
