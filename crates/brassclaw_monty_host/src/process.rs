//! Bounded, private pipe transport for one isolated instance-root interpreter.
//!
//! This is not the production instance supervisor. The caller still owns the
//! database lock, durable admission, attempts, host effects and child registry.
//! No command selects a Recipe, dispatches a Tool or retries an operation.
//! Transport cancellation is fatal instance containment. The future belongs to
//! the instance actor, never to a cancellable user turn. Task cancellation must
//! fence its task/child while this actor and the unrelated root coroutines live.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{self, Read, Write},
    path::Path,
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::Duration,
};

use monty_types::{ExcType, MontyException};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
};

use crate::{
    ContinuationKey, HostAnswer, ValueBudget, VmBounds, VmFailure,
    global::{GlobalBoundary, GlobalBounds, GlobalVm, Lifecycle},
};

const PROTOCOL: u32 = 1;
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
// Leave space for the protocol wrapper under serde_json's receive depth limit
// and bound recursive serialization before it enters the parent Rust stack.
const MAX_TRANSPORT_DEPTH: usize = 64;

/// A finite physical allocator backstop, distinct from adaptive logical heap
/// policy. A response deadline interrupts compilation/native work by killing
/// this worker; it never claims an outstanding external effect has stopped.
#[derive(Clone, Copy)]
pub struct ProcessLimits {
    pub hard_memory_bytes: usize,
    pub max_frame_bytes: usize,
    pub response_timeout: Duration,
}
impl ProcessLimits {
    fn valid(self) -> bool {
        (1024..=MAX_FRAME_BYTES).contains(&self.max_frame_bytes)
            && self.hard_memory_bytes > self.max_frame_bytes
            && self.hard_memory_bytes != usize::MAX
            && !self.response_timeout.is_zero()
            && std::time::Instant::now()
                .checked_add(self.response_timeout)
                .is_some()
    }
}

/// Verified source comes from the caller's boot store, never a script fallback.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootBoot {
    pub source: String,
    pub checksum: [u8; 32],
    pub aliases: BTreeSet<String>,
    pub bounds: GlobalBounds,
    pub startup_timeout: Duration,
}

/// Exact mechanical VM operation. Payloads deliberately have no Debug output.
#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerCommand {
    Boot {
        boot: RootBoot,
    },
    Admit {
        key: ContinuationKey,
        task: Value,
    },
    Defer {
        key: ContinuationKey,
    },
    ResumeControl {
        key: ContinuationKey,
    },
    Resolve {
        key: ContinuationKey,
        answer: PortAnswer,
    },
    BeginShutdown,
    CloseWorker {
        key: ContinuationKey,
    },
}

/// Domain errors are classified by the trusted port, without placing arbitrary
/// host exception strings into Python. A fatal instance failure uses process
/// termination; it cannot masquerade as a catchable task error.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PortAnswer {
    Return { value: Value },
    DomainError { reason_kind: String },
}
impl PortAnswer {
    fn into_host(self) -> Result<HostAnswer, VmFailure> {
        match self {
            Self::Return { value } => Ok(HostAnswer::Return(value)),
            Self::DomainError { reason_kind }
                if !reason_kind.is_empty()
                    && reason_kind.len() <= 64
                    && reason_kind
                        .bytes()
                        .all(|b| b == b'_' || b.is_ascii_lowercase()) =>
            {
                Ok(HostAnswer::Raise(MontyException::new(
                    ExcType::RuntimeError,
                    Some(reason_kind),
                )))
            }
            _ => Err(VmFailure::InvalidHostArguments),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProcessBoundary {
    HostCall {
        key: ContinuationKey,
        name: String,
        args: Vec<Value>,
        kwargs: BTreeMap<String, Value>,
    },
    ControlYield {
        key: ContinuationKey,
    },
    Waiting {
        keys: Vec<ContinuationKey>,
    },
    Stopped,
}
impl From<GlobalBoundary> for ProcessBoundary {
    fn from(boundary: GlobalBoundary) -> Self {
        match boundary {
            GlobalBoundary::HostCall(call) => Self::HostCall {
                key: call.continuation,
                name: call.name,
                args: call.args,
                kwargs: call.kwargs,
            },
            GlobalBoundary::ControlYield(key) => Self::ControlYield { key },
            GlobalBoundary::Waiting(keys) => Self::Waiting { keys },
            GlobalBoundary::Stopped => Self::Stopped,
        }
    }
}

/// VM-local state only. Pending keys remain external-reconciliation evidence;
/// even a Stopped worker cannot establish provider/process effect quiescence.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessSnapshot {
    pub lifecycle: Lifecycle,
    pub boundary: Option<ProcessBoundary>,
    pub work_waits: Vec<(u32, ContinuationKey)>,
    pub outstanding: Vec<ContinuationKey>,
    pub stdout: String,
}
impl fmt::Debug for ProcessSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessSnapshot")
            .field("lifecycle", &self.lifecycle)
            .field("work_waits", &self.work_waits.len())
            .field("outstanding", &self.outstanding.len())
            .finish_non_exhaustive()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u32,
    sequence: u64,
    command: WorkerCommand,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    protocol: u32,
    sequence: u64,
    snapshot: ProcessSnapshot,
    failure: Option<VmFailure>,
    diagnostic: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessFailure {
    InvalidLimits,
    InvalidExecutable,
    FrameLimit,
    ValueLimit,
    Spawn,
    Transport,
    Protocol,
    Deadline,
    Terminal,
    Vm(VmFailure),
}
/// Full evidence stays with the caller. Never log raw commands, stdout or VM
/// diagnostics, and never replay a retained command merely because IPC failed.
pub struct ProcessError {
    pub kind: ProcessFailure,
    pub command: Option<Box<WorkerCommand>>,
    pub snapshot: Option<Box<ProcessSnapshot>>,
    pub diagnostic: Option<String>,
    pub exit_status: Option<ExitStatus>,
}
impl ProcessError {
    fn new(kind: ProcessFailure) -> Self {
        Self {
            kind,
            command: None,
            snapshot: None,
            diagnostic: None,
            exit_status: None,
        }
    }
}
impl fmt::Debug for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Monty worker failed: {:?}", self.kind)
    }
}
impl std::error::Error for ProcessError {}

/// A single privately spawned worker. Mutable access serializes IPC only; the
/// Python root owns its concurrent task coroutines and all Recipe sequencing.
pub struct GlobalProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    limits: ProcessLimits,
    values: VmBounds,
    sequence: u64,
    terminal: bool,
    stopped: bool,
    interrupted_command: Option<Box<WorkerCommand>>,
    kill_error: Option<io::Error>,
}
impl GlobalProcess {
    pub async fn start(
        executable: &Path,
        boot: RootBoot,
        limits: ProcessLimits,
    ) -> Result<(Self, ProcessSnapshot), ProcessError> {
        if !limits.valid() {
            return Err(ProcessError::new(ProcessFailure::InvalidLimits));
        }
        if !executable.is_absolute() {
            return Err(ProcessError::new(ProcessFailure::InvalidExecutable));
        }
        // Spawn a known executable directly, never a shell or caller-authored
        // command. Preserve allocator stderr diagnostics outside framed stdout.
        let mut child = Command::new(executable)
            // No provider keys, claim tokens or ambient executable/library
            // search configuration belong in this interpreter's environment.
            .env_clear()
            .arg(limits.hard_memory_bytes.to_string())
            .arg(limits.max_frame_bytes.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| ProcessError::new(ProcessFailure::Spawn))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let workers = boot.bounds.workers;
        let values = boot.bounds.values;
        let mut process = Self {
            child,
            stdin,
            stdout,
            limits,
            values,
            sequence: 0,
            terminal: false,
            stopped: false,
            interrupted_command: None,
            kill_error: None,
        };
        let snapshot = match process.exchange(WorkerCommand::Boot { boot }).await {
            Ok(snapshot) => snapshot,
            Err(mut error) => {
                if error.exit_status.is_none() {
                    error.exit_status = process.terminate().await;
                }
                return Err(error);
            }
        };
        if snapshot.lifecycle != Lifecycle::Ready
            || snapshot.work_waits.len() != workers as usize
            || snapshot
                .work_waits
                .iter()
                .enumerate()
                .any(|(index, (id, _))| *id as usize != index)
            || snapshot.outstanding.len() != workers as usize
            || !matches!(snapshot.boundary, Some(ProcessBoundary::Waiting { .. }))
        {
            let mut error = ProcessError::new(ProcessFailure::Protocol);
            error.snapshot = Some(Box::new(snapshot));
            error.exit_status = process.terminate().await;
            return Err(error);
        }
        Ok((process, snapshot))
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Evidence retained when a caller drops an in-flight exchange future.
    /// Taking it never authorizes replay or proves an external effect settled.
    pub fn take_interrupted_command(&mut self) -> Option<Box<WorkerCommand>> {
        self.interrupted_command.take()
    }

    /// Retain an OS kill failure instead of claiming containment. An observed
    /// reaped exit remains separate evidence that local termination occurred.
    pub fn take_containment_error(&mut self) -> Option<io::Error> {
        self.kill_error.take()
    }

    pub async fn exchange(
        &mut self,
        command: WorkerCommand,
    ) -> Result<ProcessSnapshot, ProcessError> {
        if self.terminal {
            let mut error = ProcessError::new(ProcessFailure::Terminal);
            error.command = Some(Box::new(command));
            return Err(error);
        }
        let data = match &command {
            WorkerCommand::Admit { task, .. } => Some(task),
            WorkerCommand::Resolve {
                answer: PortAnswer::Return { value },
                ..
            } => Some(value),
            _ => None,
        };
        if data
            .is_some_and(|value| !transport_value(value, self.values, self.limits.max_frame_bytes))
        {
            let mut error = ProcessError::new(ProcessFailure::ValueLimit);
            error.command = Some(Box::new(command));
            return Err(error);
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.interrupt();
            let mut error = ProcessError::new(ProcessFailure::Protocol);
            error.command = Some(Box::new(command));
            return Err(error);
        };
        let request = Request {
            protocol: PROTOCOL,
            sequence,
            command,
        };
        let frame = match encode(&request, self.limits.max_frame_bytes) {
            Ok(frame) => frame,
            Err(_) => {
                let mut error = ProcessError::new(ProcessFailure::FrameLimit);
                error.command = Some(Box::new(request.command));
                return Err(error);
            }
        };
        let limit = self.limits.max_frame_bytes;
        let timeout = self.limits.response_timeout;
        // Dropping this future after writing any bytes kills the worker. Its
        // partially advanced VM cannot accept a later request as a fresh call.
        let mut guard = ExchangeGuard {
            owner: self,
            armed: true,
            request: Some(request),
        };
        let operation = async {
            let stdin = guard.owner.stdin.as_mut().ok_or(ProcessFailure::Terminal)?;
            stdin
                .write_all(&(frame.len() as u32).to_be_bytes())
                .await
                .map_err(|_| ProcessFailure::Transport)?;
            stdin
                .write_all(&frame)
                .await
                .map_err(|_| ProcessFailure::Transport)?;
            stdin.flush().await.map_err(|_| ProcessFailure::Transport)?;
            let stdout = guard
                .owner
                .stdout
                .as_mut()
                .ok_or(ProcessFailure::Terminal)?;
            let length = stdout
                .read_u32()
                .await
                .map_err(|_| ProcessFailure::Transport)? as usize;
            if length == 0 || length > limit {
                return Err(ProcessFailure::Protocol);
            }
            let mut bytes = vec![0; length];
            stdout
                .read_exact(&mut bytes)
                .await
                .map_err(|_| ProcessFailure::Transport)?;
            serde_json::from_slice::<Reply>(&bytes).map_err(|_| ProcessFailure::Protocol)
        };
        let reply = match tokio::time::timeout(timeout, operation).await {
            Ok(Ok(reply)) if reply.protocol == PROTOCOL && reply.sequence == sequence => reply,
            other => {
                let kind = match other {
                    Err(_) => ProcessFailure::Deadline,
                    Ok(Err(kind)) => kind,
                    Ok(Ok(_)) => ProcessFailure::Protocol,
                };
                guard.owner.interrupt();
                guard.armed = false;
                let mut error = ProcessError::new(kind);
                error.command = guard
                    .request
                    .take()
                    .map(|request| Box::new(request.command));
                error.exit_status = guard.owner.terminate().await;
                return Err(error);
            }
        };
        guard.owner.sequence = sequence;
        guard.armed = false;
        if let Some(failure) = reply.failure {
            let mut error = ProcessError::new(ProcessFailure::Vm(failure));
            error.command = guard
                .request
                .take()
                .map(|request| Box::new(request.command));
            error.snapshot = Some(Box::new(reply.snapshot));
            error.diagnostic = reply.diagnostic;
            return Err(error);
        }
        if reply.snapshot.lifecycle == Lifecycle::Stopped {
            guard.owner.stopped = true;
            guard.owner.terminal = true;
            guard.owner.stdin = None;
            guard.owner.stdout = None;
        }
        Ok(reply.snapshot)
    }

    fn interrupt(&mut self) {
        self.terminal = true;
        self.stdin = None;
        self.stdout = None;
        // Exit racing kill is normal; only a reaped exit below acknowledges
        // local termination. Never turn a failed kill into a stopped receipt.
        if let Err(error) = self.child.start_kill() {
            self.kill_error.get_or_insert(error);
        }
    }

    /// Fatal containment, not task cancellation or external-effect settlement.
    /// None means local exit was not acknowledged within the response bound.
    pub async fn terminate(&mut self) -> Option<ExitStatus> {
        self.interrupt();
        tokio::time::timeout(self.limits.response_timeout, self.child.wait())
            .await
            .ok()
            .and_then(Result::ok)
    }

    /// Reap only after the actual VM shutdown acknowledgement. Wait normally
    /// before killing so a successfully draining worker has time to exit.
    pub async fn wait_stopped(&mut self) -> Result<ExitStatus, ProcessError> {
        if !self.stopped {
            return Err(ProcessError::new(ProcessFailure::Terminal));
        }
        match tokio::time::timeout(self.limits.response_timeout, self.child.wait()).await {
            Ok(Ok(status)) => Ok(status),
            result => {
                let mut error = ProcessError::new(if result.is_err() {
                    ProcessFailure::Deadline
                } else {
                    ProcessFailure::Transport
                });
                error.exit_status = self.terminate().await;
                Err(error)
            }
        }
    }
}
struct ExchangeGuard<'a> {
    owner: &'a mut GlobalProcess,
    armed: bool,
    request: Option<Request>,
}
impl Drop for ExchangeGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.owner.interrupted_command =
                self.request.take().map(|request| Box::new(request.command));
            self.owner.interrupt();
        }
    }
}

// Bounded serialization must fail before allocating an unbounded intermediate
// buffer. It does not silently truncate either commands or returned evidence.
struct Frame {
    bytes: Vec<u8>,
    limit: usize,
}
fn transport_value(value: &Value, mut bounds: VmBounds, frame_limit: usize) -> bool {
    bounds.max_value_depth = bounds.max_value_depth.min(MAX_TRANSPORT_DEPTH);
    bounds.max_value_nodes = bounds.max_value_nodes.min(frame_limit);
    bounds.max_value_bytes = bounds.max_value_bytes.min(frame_limit);
    let mut budget = ValueBudget::new(bounds, VmFailure::InvalidInputs);
    let mut pending = vec![(value, 0usize)];
    while let Some((value, depth)) = pending.pop() {
        if budget.node(depth).is_err() {
            return false;
        }
        match value {
            Value::String(value) => {
                if budget.bytes(value.len()).is_err() {
                    return false;
                }
            }
            Value::Array(values) => {
                // Refuse a too-wide graph before growing the traversal stack.
                if pending
                    .len()
                    .checked_add(values.len())
                    .is_none_or(|count| count > bounds.max_value_nodes - budget.nodes)
                {
                    return false;
                }
                pending.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Value::Object(values) => {
                if pending
                    .len()
                    .checked_add(values.len())
                    .is_none_or(|count| count > bounds.max_value_nodes - budget.nodes)
                {
                    return false;
                }
                for (key, value) in values {
                    if budget.bytes(key.len()).is_err() {
                        return false;
                    }
                    pending.push((value, depth + 1));
                }
            }
            _ => {}
        }
    }
    true
}
impl Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > self.limit)
        {
            return Err(io::Error::other("frame limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize, limit: usize) -> io::Result<Vec<u8>> {
    let mut frame = Frame {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut frame, value).map_err(io::Error::other)?;
    Ok(frame.bytes)
}
fn read_frame<T: DeserializeOwned>(input: &mut impl Read, limit: usize) -> io::Result<T> {
    let mut header = [0; 4];
    input.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > limit {
        return Err(io::Error::other("invalid frame length"));
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
fn write_frame(output: &mut impl Write, reply: &Reply, limit: usize) -> io::Result<()> {
    let bytes = encode(reply, limit)?;
    output.write_all(&(bytes.len() as u32).to_be_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}

/// Used only by the worker binary that installs LimitedAllocator. Any malformed
/// protocol frame ends that process, rather than resynchronizing or replaying.
pub fn worker_main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let hard_memory_bytes: usize = arguments.next().ok_or("missing memory limit")?.parse()?;
    let max_frame_bytes: usize = arguments.next().ok_or("missing frame limit")?.parse()?;
    if arguments.next().is_some()
        || !(1024..=MAX_FRAME_BYTES).contains(&max_frame_bytes)
        || hard_memory_bytes <= max_frame_bytes
        || hard_memory_bytes == usize::MAX
    {
        return Err("invalid worker limits".into());
    }
    // Arm before receiving/compiling source. Never reset this ceiling per chat,
    // task or IPC command, and never disable it after the initial boot.
    monty_alloc::set_hard_limit(Some(hard_memory_bytes))?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut vm: Option<GlobalVm> = None;
    let mut sequence = 0u64;
    loop {
        let request: Request = read_frame(&mut input, max_frame_bytes)
            .map_err(|_| io::Error::other("invalid worker request"))?;
        if request.protocol != PROTOCOL || sequence.checked_add(1) != Some(request.sequence) {
            return Err("invalid worker sequence".into());
        }
        sequence = request.sequence;
        let result = match request.command {
            WorkerCommand::Boot { boot } if vm.is_none() && sequence == 1 => GlobalVm::start_ready(
                Arc::from(boot.source),
                boot.checksum,
                boot.aliases,
                boot.bounds,
                boot.startup_timeout,
            )
            .map(|root| {
                let boundary = GlobalBoundary::Waiting(root.outstanding());
                vm = Some(root);
                Some(boundary)
            }),
            command => match vm.as_mut() {
                Some(root) => match command {
                    WorkerCommand::Admit { key, task } => root.admit(key, task).map(Some),
                    WorkerCommand::Defer { key } => root.defer(key).map(Some),
                    WorkerCommand::ResumeControl { key } => root.resume_control(key).map(Some),
                    WorkerCommand::Resolve { key, answer } => answer
                        .into_host()
                        .map_err(crate::VmError::kind)
                        .and_then(|answer| root.resolve(key, answer))
                        .map(Some),
                    WorkerCommand::BeginShutdown => root.begin_shutdown().map(|()| None),
                    WorkerCommand::CloseWorker { key } => root.close_worker(key).map(Some),
                    WorkerCommand::Boot { .. } => {
                        Err(crate::VmError::kind(VmFailure::WrongBoundary))
                    }
                },
                None => Err(crate::VmError::kind(VmFailure::Terminal)),
            },
        };
        let (boundary, failure, diagnostic, failed_stdout) = match result {
            Ok(boundary) => (
                boundary.map(ProcessBoundary::from),
                None,
                None,
                String::new(),
            ),
            Err(error) => (
                None,
                Some(error.failure),
                error.exception.map(|error| error.to_string()),
                error.stdout,
            ),
        };
        let snapshot = match vm.as_mut() {
            Some(root) => ProcessSnapshot {
                lifecycle: root.lifecycle(),
                boundary,
                work_waits: root.work_waits(),
                outstanding: root.outstanding(),
                stdout: format!("{failed_stdout}{}", root.take_stdout()),
            },
            None => ProcessSnapshot {
                lifecycle: Lifecycle::Failed,
                boundary,
                work_waits: Vec::new(),
                outstanding: Vec::new(),
                stdout: failed_stdout,
            },
        };
        let stopped = snapshot.lifecycle == Lifecycle::Stopped;
        write_frame(
            &mut output,
            &Reply {
                protocol: PROTOCOL,
                sequence,
                snapshot,
                failure,
                diagnostic,
            },
            max_frame_bytes,
        )
        .map_err(|_| io::Error::other("worker reply failed"))?;
        if stopped {
            return Ok(());
        }
    }
}
