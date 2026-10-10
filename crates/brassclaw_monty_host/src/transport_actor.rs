//! Instance-owned IPC, independent of cancellable turn futures. This actor
//! serializes transport only; it never selects a step, dispatches a Tool,
//! retries work or resolves a Monty port autonomously.
use std::{
    collections::BTreeMap,
    fmt, io,
    panic::AssertUnwindSafe,
    path::Path,
    process::ExitStatus,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{FutureExt, future::BoxFuture};

use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    VmBounds,
    global::Lifecycle,
    heap::HeapSettings,
    process::{
        GlobalProcess, ProcessError, ProcessFailure, ProcessLimits, ProcessSnapshot, RecipeCommand,
        RootBoot, WorkerCommand, recover_command, retain_command,
    },
};

type ExchangeResult = Result<ProcessSnapshot, ProcessError>;

/// All IPC phases of an owned transaction retain the same accepted frame
/// contract. The instance receive/allocator capacity is a separate policy.
struct RetainedExchange<'a> {
    process: &'a mut GlobalProcess,
    max_frame_bytes: usize,
}
impl RetainedExchange<'_> {
    async fn exchange(&mut self, command: WorkerCommand) -> ExchangeResult {
        self.process
            .exchange_with_frame_limit(command, self.max_frame_bytes)
            .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorLimits {
    pub max_unclaimed: usize,
    /// Reserved command + maximum response frame bytes, not a physical heap
    /// measurement. Decoded values and bookkeeping have additional overhead.
    pub max_reserved_frame_bytes: usize,
    pub max_control_unclaimed: usize,
    pub max_control_reserved_frame_bytes: usize,
}
impl ActorLimits {
    fn valid(self, frame: usize) -> bool {
        self.max_unclaimed > 0
            && u32::try_from(self.max_unclaimed).is_ok()
            && self.max_reserved_frame_bytes > frame
            && self.max_control_unclaimed > 0
            && u32::try_from(self.max_control_unclaimed).is_ok()
            && frame
                .checked_mul(2)
                .is_some_and(|minimum| self.max_control_reserved_frame_bytes >= minimum)
    }
    fn lane(self, control: bool) -> (usize, usize) {
        if control {
            (
                self.max_control_unclaimed,
                self.max_control_reserved_frame_bytes,
            )
        } else {
            (self.max_unclaimed, self.max_reserved_frame_bytes)
        }
    }
}
/// Shared policy and credited frame requirements. This describes ownership;
/// it neither measures memory nor publishes worker allocator limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRequirements {
    pub configured_frame_bytes: usize,
    pub max_retained_frame_bytes: usize,
}
impl FrameRequirements {
    /// Reservation requirement only; does not acknowledge allocator uptake.
    pub fn required_capacity(self) -> usize {
        self.configured_frame_bytes
            .max(self.max_retained_frame_bytes)
    }
}

/// Counts cover queued, executing and completed-but-unclaimed requests. Each
/// lane has independent credits, so ordinary saturation cannot consume control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorCapacityObservation {
    pub limits: ActorLimits,
    /// Acknowledged default for new exchanges, not an old receipt's bound.
    pub configured_frame_bytes: usize,
    /// Largest credited bound, including completed unclaimed receipts.
    pub max_retained_frame_bytes: usize,
    pub ordinary_requests: usize,
    pub ordinary_reserved_bytes: usize,
    pub control_requests: usize,
    pub control_reserved_bytes: usize,
}
impl ActorCapacityObservation {
    /// Minimum receive/reserve capacity needed by current policy and credits.
    /// This is neither measured memory nor an acknowledged allocator change.
    pub fn required_frame_capacity(self) -> usize {
        self.configured_frame_bytes
            .max(self.max_retained_frame_bytes)
    }
    pub fn ordinary_over_capacity(self) -> bool {
        self.ordinary_requests > self.limits.max_unclaimed
            || self.ordinary_reserved_bytes > self.limits.max_reserved_frame_bytes
    }
    pub fn control_over_capacity(self) -> bool {
        self.control_requests > self.limits.max_control_unclaimed
            || self.control_reserved_bytes > self.limits.max_control_reserved_frame_bytes
    }
}
/// Host lifecycle/control deadlines, not Monty task compute accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostingDeadlines {
    pub startup_timeout: Duration,
    pub response_timeout: Duration,
}
impl HostingDeadlines {
    pub fn valid(self) -> bool {
        !self.startup_timeout.is_zero()
            && self
                .startup_timeout
                .subsec_nanos()
                .is_multiple_of(1_000_000)
            && self
                .response_timeout
                .subsec_nanos()
                .is_multiple_of(1_000_000)
            && self.startup_timeout.as_millis() <= u64::MAX as u128
            && self.response_timeout.as_millis() <= u64::MAX as u128
            && self.response_timeout >= self.startup_timeout
            && std::time::Instant::now()
                .checked_add(self.response_timeout)
                .is_some()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequestId(Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorFailure {
    Backpressure,
    Closed,
    AccountingUnavailable,
    UnknownRequest,
    NotReady,
    InvalidLimits,
    RuntimeUnavailable,
    Join,
    Transport(ProcessFailure),
}
impl fmt::Display for ActorFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Monty transport actor failed: {self:?}")
    }
}
impl std::error::Error for ActorFailure {}

pub struct SubmitError {
    pub kind: ActorFailure,
    pub command: Box<WorkerCommand>,
}
impl fmt::Debug for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubmitError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}
impl std::error::Error for SubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}

#[derive(Debug)]
pub enum StartError {
    Actor(ActorFailure),
    Worker(ProcessError),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Actor(error) => error.fmt(f),
            Self::Worker(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Actor(error) => Some(error),
            Self::Worker(error) => Some(error),
        }
    }
}

struct Record {
    response_timeout: Duration,
    max_frame_bytes: usize,
    wire: Vec<u8>,
    reserved: usize,
    transport_started: bool,
    control: bool,
    outcome: Option<ExchangeResult>,
    ready: watch::Sender<bool>,
}
#[derive(Default)]
struct Credits {
    count: usize,
    reserved: usize,
}
struct Ledger {
    values: VmBounds,
    deadlines: HostingDeadlines,
    limits: ActorLimits,
    frame: watch::Sender<FrameRequirements>,
    retained_frames: BTreeMap<usize, usize>,
    records: BTreeMap<RequestId, Record>,
    ordinary: Credits,
    control: Credits,
    closed: bool,
}
impl Ledger {
    fn publish_frame_requirement(&self) {
        let before = *self.frame.borrow();
        let max_retained_frame_bytes = self
            .retained_frames
            .last_key_value()
            .map_or(0, |(frame, _)| *frame);
        if before.max_retained_frame_bytes != max_retained_frame_bytes {
            self.frame.send_replace(FrameRequirements {
                max_retained_frame_bytes,
                ..before
            });
        }
    }
    fn refund_record(&mut self, id: RequestId) -> Result<Record, ActorFailure> {
        let record = self.records.get(&id).ok_or(ActorFailure::UnknownRequest)?;
        let frame = record.max_frame_bytes;
        let control = record.control;
        let remaining_frames = self
            .retained_frames
            .get(&frame)
            .and_then(|count| count.checked_sub(1))
            .ok_or(ActorFailure::AccountingUnavailable)?;
        let credits = self.credits(control);
        let remaining_count = credits
            .count
            .checked_sub(1)
            .ok_or(ActorFailure::AccountingUnavailable)?;
        let remaining_bytes = credits
            .reserved
            .checked_sub(record.reserved)
            .ok_or(ActorFailure::AccountingUnavailable)?;
        // Validate the whole refund before transferring any evidence ownership.
        let record = self.records.remove(&id).expect("checked record");
        if remaining_frames == 0 {
            self.retained_frames.remove(&frame);
        } else {
            self.retained_frames.insert(frame, remaining_frames);
        }
        let credits = self.credits_mut(control);
        credits.count = remaining_count;
        credits.reserved = remaining_bytes;
        self.publish_frame_requirement();
        Ok(record)
    }
    fn credits(&self, control: bool) -> &Credits {
        if control {
            &self.control
        } else {
            &self.ordinary
        }
    }
    fn credits_mut(&mut self, control: bool) -> &mut Credits {
        if control {
            &mut self.control
        } else {
            &mut self.ordinary
        }
    }
}
#[derive(Clone)]
pub struct CompletionInbox {
    ledger: Arc<Mutex<Ledger>>,
}

/// Keep private. Debug output contains only routing/status, never source,
/// messages, host returns, stdout or credentials. Receipt collection is not
/// authorization to replay a command or evidence of external quiescence.
pub struct TransportReceipt {
    pub id: RequestId,
    pub transport_started: bool,
    pub response_timeout: Duration,
    pub max_frame_bytes: usize,
    wire: Vec<u8>,
    pub outcome: ExchangeResult,
}
impl fmt::Debug for TransportReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportReceipt")
            .field("id", &self.id)
            .field("transport_started", &self.transport_started)
            .field("succeeded", &self.outcome.is_ok())
            .finish_non_exhaustive()
    }
}
impl TransportReceipt {
    pub fn original_command(&self) -> Result<WorkerCommand, ActorFailure> {
        recover_command(&self.wire).map_err(ActorFailure::Transport)
    }
}
impl CompletionInbox {
    pub fn outstanding(&self) -> Result<Vec<RequestId>, ActorFailure> {
        Ok(self
            .ledger
            .lock()
            .map_err(|_| ActorFailure::AccountingUnavailable)?
            .records
            .keys()
            .copied()
            .collect())
    }
    pub fn try_take(&self, id: RequestId) -> Result<TransportReceipt, ActorFailure> {
        let mut ledger = self
            .ledger
            .lock()
            .map_err(|_| ActorFailure::AccountingUnavailable)?;
        let record = ledger
            .records
            .get(&id)
            .ok_or(ActorFailure::UnknownRequest)?;
        if record.outcome.is_none() {
            return Err(ActorFailure::NotReady);
        }
        let mut record = ledger.refund_record(id)?;
        Ok(TransportReceipt {
            id,
            transport_started: record.transport_started,
            response_timeout: record.response_timeout,
            max_frame_bytes: record.max_frame_bytes,
            wire: record.wire,
            outcome: record.outcome.take().expect("checked outcome"),
        })
    }
    pub async fn ready(&self, id: RequestId) -> Result<(), ActorFailure> {
        let mut ready = {
            let ledger = self
                .ledger
                .lock()
                .map_err(|_| ActorFailure::AccountingUnavailable)?;
            ledger
                .records
                .get(&id)
                .ok_or(ActorFailure::UnknownRequest)?
                .ready
                .subscribe()
        };
        // watch preserves notification across subscription/check races and
        // multiple observers. Exactly one observer can collect the receipt.
        while !*ready.borrow_and_update() {
            ready.changed().await.map_err(|_| ActorFailure::Closed)?;
        }
        Ok(())
    }
    pub async fn wait(&self, id: RequestId) -> Result<TransportReceipt, ActorFailure> {
        self.ready(id).await?;
        self.try_take(id)
    }
}
pub struct RequestTicket {
    pub id: RequestId,
    inbox: CompletionInbox,
}
impl RequestTicket {
    pub async fn wait(&self) -> Result<TransportReceipt, ActorFailure> {
        self.inbox.wait(self.id).await
    }
}
/// Result of an installation-owned durable settings commit, never a Tool grant.
pub enum HeapCommitOutcome {
    Committed,
    Rejected,
    Unknown,
}
/// Installation-owned durable settings operation. Its independent deadline is
/// captured at acceptance; an IPC response deadline does not retime a DB write.
pub struct DurableSettingsCommit {
    pub future: BoxFuture<'static, HeapCommitOutcome>,
    pub timeout: Duration,
    /// Optional manual heap successor, applied with the full settings ACK.
    pub heap_update: Option<crate::heap::HeapUpdate>,
}
struct HeapCommit {
    future: BoxFuture<'static, HeapCommitOutcome>,
    timeout: Duration,
}
struct Envelope {
    response_timeout: Duration,
    max_frame_bytes: usize,
    heap_commit: Option<HeapCommit>,
    runtime_commit: Option<RuntimeCommit>,
    durable_settings: Option<DurableSettingsCommit>,
    id: RequestId,
    command: WorkerCommand,
}
/// Installation-owned, synchronous Rust policy publication after the exact
/// worker ACK and before another exchange can begin. No database/model/Tool
/// work or task sequencing belongs in this callback. Failure after the worker
/// ACK fences the instance and retains that ACK; it never rolls back or retries.
pub type RuntimeCommit =
    Box<dyn FnOnce(&ProcessSnapshot) -> Result<(), ActorFailure> + Send + 'static>;
#[derive(Clone)]
pub struct TransportClient {
    tx: mpsc::UnboundedSender<Envelope>,
    control_tx: mpsc::UnboundedSender<Envelope>,
    inbox: CompletionInbox,
    values: watch::Receiver<VmBounds>,
    recipe_contexts: watch::Receiver<crate::process::RecipeContextCapacity>,
    memory: watch::Receiver<crate::heap::WorkerMemoryStatus>,
    frame: watch::Receiver<FrameRequirements>,
    stopped: watch::Receiver<Option<StopKind>>,
}
impl TransportClient {
    pub fn validate_limits(&self, limits: ActorLimits) -> Result<(), ActorFailure> {
        self.validate_limits_for_frame(limits, self.frame_limit())
    }
    pub fn validate_limits_for_frame(
        &self,
        limits: ActorLimits,
        frame: usize,
    ) -> Result<(), ActorFailure> {
        if crate::process::valid_frame_limit(frame) && limits.valid(frame) {
            Ok(())
        } else {
            Err(ActorFailure::InvalidLimits)
        }
    }
    pub fn capacity(&self) -> Result<ActorCapacityObservation, ActorFailure> {
        let ledger = self
            .inbox
            .ledger
            .lock()
            .map_err(|_| ActorFailure::AccountingUnavailable)?;
        let configured_frame_bytes = ledger.frame.borrow().configured_frame_bytes;
        Ok(ActorCapacityObservation {
            limits: ledger.limits,
            configured_frame_bytes,
            max_retained_frame_bytes: ledger
                .retained_frames
                .last_key_value()
                .map_or(0, |(frame, _)| *frame),
            ordinary_requests: ledger.ordinary.count,
            ordinary_reserved_bytes: ledger.ordinary.reserved,
            control_requests: ledger.control.count,
            control_reserved_bytes: ledger.control.reserved,
        })
    }
    /// Trusted host policy publication after the shared worker revision ACK.
    /// Reductions keep every existing credit; refunds drain capacity debt.
    pub fn publish_limits(&self, limits: ActorLimits) -> Result<(), ActorFailure> {
        self.publish_hosting_policy(Some(limits), None)
    }
    /// Live slice increases cannot invalidate older accepted exchanges. The
    /// current deadline also bounds every new acceptance before worker ACK.
    pub fn validate_execution_slice(&self, slice: Duration) -> Result<(), ActorFailure> {
        let ledger = self
            .inbox
            .ledger
            .lock()
            .map_err(|_| ActorFailure::AccountingUnavailable)?;
        if ledger.closed {
            return Err(ActorFailure::Closed);
        }
        if slice >= ledger.deadlines.response_timeout
            || ledger
                .records
                .values()
                .any(|record| record.outcome.is_none() && slice >= record.response_timeout)
        {
            return Err(ActorFailure::Backpressure);
        }
        Ok(())
    }

    pub fn hosting_deadlines(&self) -> Result<HostingDeadlines, ActorFailure> {
        Ok(self
            .inbox
            .ledger
            .lock()
            .map_err(|_| ActorFailure::AccountingUnavailable)?
            .deadlines)
    }
    /// Publication is atomic with acceptance into both lanes. Requests accepted
    /// earlier retain their own response deadline, including queued requests.
    pub fn publish_hosting_policy(
        &self,
        limits: Option<ActorLimits>,
        deadlines: Option<HostingDeadlines>,
    ) -> Result<(), ActorFailure> {
        self.publish_hosting_policy_for_bounds(limits, deadlines, *self.values.borrow(), None)
    }

    pub(crate) fn publish_hosting_policy_for_bounds(
        &self,
        limits: Option<ActorLimits>,
        deadlines: Option<HostingDeadlines>,
        values: VmBounds,
        memory: Option<crate::heap::WorkerMemoryStatus>,
    ) -> Result<(), ActorFailure> {
        if deadlines.is_some_and(|deadlines| {
            !deadlines.valid() || values.execution_slice >= deadlines.response_timeout
        }) {
            return Err(ActorFailure::InvalidLimits);
        }
        let mut ledger = self
            .inbox
            .ledger
            .lock()
            .map_err(|_| ActorFailure::AccountingUnavailable)?;
        if ledger.closed {
            return Err(ActorFailure::Closed);
        }
        let before = *ledger.frame.borrow();
        let configured = memory.map_or(before.configured_frame_bytes, |memory| {
            memory.frame.configured_frame_bytes
        });
        let retained = ledger
            .retained_frames
            .last_key_value()
            .map_or(0, |(frame, _)| *frame);
        if !limits.unwrap_or(ledger.limits).valid(configured)
            || memory.is_some_and(|memory| {
                !memory.frame.valid() || memory.frame.effective_capacity_bytes < retained
            })
        {
            return Err(ActorFailure::InvalidLimits);
        }
        if let Some(limits) = limits {
            ledger.limits = limits;
        }
        if let Some(deadlines) = deadlines {
            ledger.deadlines = deadlines;
        }
        if memory.is_some() {
            let next = FrameRequirements {
                configured_frame_bytes: configured,
                max_retained_frame_bytes: retained,
            };
            if before != next {
                ledger.frame.send_replace(next);
            }
            ledger.values = values;
            // Receiver-only clients cannot publish. Owner observations follow
            // immediately after this callback while no later RPC can advance.
        }
        Ok(())
    }
    pub(crate) fn frame_limit(&self) -> usize {
        self.frame.borrow().configured_frame_bytes
    }
    /// Read-only, event-driven requirements; keeps no transport/service owner.
    /// Copy a borrowed observation before awaiting or collecting a receipt.
    pub fn frame_requirements(&self) -> watch::Receiver<FrameRequirements> {
        self.frame.clone()
    }
    pub fn live_vm_bounds(&self) -> watch::Receiver<VmBounds> {
        self.values.clone()
    }

    pub fn live_recipe_contexts(&self) -> watch::Receiver<crate::process::RecipeContextCapacity> {
        self.recipe_contexts.clone()
    }

    pub fn allocator_status(&self) -> crate::heap::AllocatorStatus {
        self.memory.borrow().allocator
    }

    pub fn frame_status(&self) -> crate::heap::FrameStatus {
        self.memory.borrow().frame
    }
    pub fn worker_memory_status(&self) -> crate::heap::WorkerMemoryStatus {
        *self.memory.borrow()
    }

    /// Read-only acknowledged worker capacity; copy observations before awaits.
    pub fn live_worker_memory(&self) -> watch::Receiver<crate::heap::WorkerMemoryStatus> {
        self.memory.clone()
    }

    pub fn completions(&self) -> CompletionInbox {
        self.inbox.clone()
    }
    /// Actual actor termination notification, including unsolicited idle death.
    /// No RPC, task/heap polling or successful effect acknowledgement occurs.
    pub async fn stopped(&self) -> Result<StopKind, ActorFailure> {
        let mut observed = self.stopped.clone();
        loop {
            if let Some(kind) = *observed.borrow_and_update() {
                return Ok(kind);
            }
            observed.changed().await.map_err(|_| ActorFailure::Closed)?;
        }
    }

    pub fn stop_kind(&self) -> Result<Option<StopKind>, ActorFailure> {
        let kind = *self.stopped.borrow();
        if kind.is_none() && self.stopped.has_changed().is_err() {
            return Err(ActorFailure::Closed);
        }
        Ok(kind)
    }
    /// Synchronous acceptance: cancellation cannot occur between publishing the
    /// receipt identity and sending its command. Full/closed queues return the
    /// whole unaccepted command. Credits include abandoned/completed requests.
    pub fn try_submit(&self, command: WorkerCommand) -> Result<RequestTicket, SubmitError> {
        self.try_submit_inner(command, None, None, None, None)
    }

    /// Optional tighter bound for one exchange; does not publish instance
    /// policy. Queueing, credit reservation and worker response keep this exact
    /// bound, including when the caller abandons its ticket.
    pub fn try_submit_with_frame_limit(
        &self,
        command: WorkerCommand,
        max_frame_bytes: usize,
    ) -> Result<RequestTicket, SubmitError> {
        self.try_submit_inner(command, None, None, None, Some(max_frame_bytes))
    }

    /// The instance service owns both the accepted command and its bounded
    /// synchronous callback. Dropping a waiter cannot cancel either. Only
    /// mechanical settings commands qualify; this is not a general execution
    /// callback or a Tool permission boundary.
    pub fn try_submit_runtime_publication(
        &self,
        command: WorkerCommand,
        commit: RuntimeCommit,
    ) -> Result<RequestTicket, SubmitError> {
        if !matches!(
            &command,
            WorkerCommand::Recipe {
                command: RecipeCommand::UpdateSettings { .. }
                    | RecipeCommand::UpdateRuntimeSettings { .. }
            }
        ) {
            return Err(SubmitError {
                kind: ActorFailure::InvalidLimits,
                command: Box::new(command),
            });
        }
        self.try_submit_inner(command, None, Some(commit), None, None)
    }

    pub(crate) fn try_submit_runtime_transaction(
        &self,
        command: WorkerCommand,
        commit: RuntimeCommit,
        durable: DurableSettingsCommit,
    ) -> Result<RequestTicket, SubmitError> {
        let heap_update = match &command {
            WorkerCommand::Recipe {
                command: RecipeCommand::UpdateRuntimeSettings { heap_update, .. },
            } => *heap_update,
            _ => None,
        };
        if !matches!(
            &command,
            WorkerCommand::Recipe {
                command: RecipeCommand::UpdateRuntimeSettings { .. }
                    | RecipeCommand::UpdateSettings { .. }
            }
        ) || heap_update != durable.heap_update
            || durable.timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(durable.timeout)
                .is_none()
        {
            return Err(SubmitError {
                kind: ActorFailure::InvalidLimits,
                command: Box::new(command),
            });
        }
        self.try_submit_inner(command, None, Some(commit), Some(durable), None)
    }
    pub(crate) fn try_submit_heap_transaction(
        &self,
        expected_revision: u64,
        settings: HeapSettings,
        commit: BoxFuture<'static, HeapCommitOutcome>,
        timeout: Duration,
    ) -> Result<RequestTicket, SubmitError> {
        self.try_submit_inner(
            WorkerCommand::UpdateHeap {
                expected_revision,
                settings,
                automatic: false,
            },
            Some(HeapCommit {
                future: commit,
                timeout,
            }),
            None,
            None,
            None,
        )
    }
    fn try_submit_inner(
        &self,
        command: WorkerCommand,
        heap_commit: Option<HeapCommit>,
        runtime_commit: Option<RuntimeCommit>,
        durable_settings: Option<DurableSettingsCommit>,
        max_frame_bytes: Option<usize>,
    ) -> Result<RequestTicket, SubmitError> {
        let submit = |kind, command| SubmitError {
            kind,
            command: Box::new(command),
        };
        let control = !matches!(
            &command,
            WorkerCommand::Admit { .. }
                | WorkerCommand::Boot { .. }
                | WorkerCommand::Recipe {
                    command: RecipeCommand::Start { .. }
                }
        );
        let ledger = match self.inbox.ledger.lock() {
            Ok(ledger) => ledger,
            Err(_) => return Err(submit(ActorFailure::AccountingUnavailable, command)),
        };
        if ledger.closed {
            return Err(submit(ActorFailure::Closed, command));
        }
        if ledger.credits(control).count >= ledger.limits.lane(control).0 {
            return Err(submit(ActorFailure::Backpressure, command));
        }
        let configured_frame = ledger.frame.borrow().configured_frame_bytes;
        let accepted_values = ledger.values;
        drop(ledger);
        let max_frame_bytes = max_frame_bytes.unwrap_or(configured_frame);
        if max_frame_bytes == 0 || max_frame_bytes > configured_frame {
            return Err(submit(ActorFailure::InvalidLimits, command));
        }
        let wire = match retain_command(&command, accepted_values, max_frame_bytes) {
            Ok(wire) => wire,
            Err(kind) => return Err(submit(ActorFailure::Transport(kind), command)),
        };
        let reserved = match wire.len().checked_add(max_frame_bytes) {
            Some(reserved) => reserved,
            None => return Err(submit(ActorFailure::Backpressure, command)),
        };
        let mut ledger = match self.inbox.ledger.lock() {
            Ok(ledger) => ledger,
            Err(_) => return Err(submit(ActorFailure::AccountingUnavailable, command)),
        };
        if ledger.closed {
            return Err(submit(ActorFailure::Closed, command));
        }
        // Expensive serialization is outside the lock. A future frame-policy
        // publication cannot accept these bytes under a different bound.
        if ledger.frame.borrow().configured_frame_bytes != configured_frame
            || ledger.values != accepted_values
        {
            return Err(submit(ActorFailure::Backpressure, command));
        }
        let required_capacity = ledger
            .retained_frames
            .last_key_value()
            .map_or(max_frame_bytes, |(frame, _)| (*frame).max(max_frame_bytes));
        match &command {
            WorkerCommand::Recipe {
                command:
                    RecipeCommand::UpdateRuntimeSettings {
                        frame_update: Some(frame),
                        ..
                    },
            } => {
                if !frame.valid()
                    || frame.required_capacity_bytes < required_capacity
                    || (runtime_commit.is_none()
                        && !ledger.limits.valid(frame.configured_frame_bytes))
                {
                    return Err(submit(ActorFailure::InvalidLimits, command));
                }
            }
            WorkerCommand::ReconcileFrameCapacity {
                configured_frame_bytes,
                required_capacity_bytes,
                ..
            } if *configured_frame_bytes != configured_frame
                || *required_capacity_bytes < required_capacity.max(configured_frame) =>
            {
                return Err(submit(ActorFailure::InvalidLimits, command));
            }
            _ => {}
        }
        if ledger.credits(control).count >= ledger.limits.lane(control).0 {
            return Err(submit(ActorFailure::Backpressure, command));
        }
        let byte_limit = ledger.limits.lane(control).1;
        let total = match ledger.credits(control).reserved.checked_add(reserved) {
            Some(total) if total <= byte_limit => total,
            _ => return Err(submit(ActorFailure::Backpressure, command)),
        };
        let id = RequestId(Uuid::new_v4());
        if ledger.records.contains_key(&id) {
            return Err(submit(ActorFailure::AccountingUnavailable, command));
        }
        let frame_count = match ledger
            .retained_frames
            .get(&max_frame_bytes)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
        {
            Some(count) => count,
            None => return Err(submit(ActorFailure::AccountingUnavailable, command)),
        };
        let response_timeout = match &command {
            WorkerCommand::Boot { boot } => {
                ledger.deadlines.response_timeout.min(boot.startup_timeout)
            }
            _ => ledger.deadlines.response_timeout,
        };
        let (ready, _) = watch::channel(false);
        ledger.records.insert(
            id,
            Record {
                response_timeout,
                max_frame_bytes,
                wire,
                reserved,
                transport_started: false,
                control,
                outcome: None,
                ready,
            },
        );
        ledger.credits_mut(control).reserved = total;
        ledger.credits_mut(control).count += 1;
        ledger.retained_frames.insert(max_frame_bytes, frame_count);
        ledger.publish_frame_requirement();
        let tx = if control { &self.control_tx } else { &self.tx };
        if let Err(error) = tx.send(Envelope {
            response_timeout,
            max_frame_bytes,
            id,
            command,
            heap_commit,
            runtime_commit,
            durable_settings,
        }) {
            if ledger.refund_record(id).is_err() {
                ledger.closed = true;
                return Err(submit(ActorFailure::AccountingUnavailable, error.0.command));
            }
            return Err(submit(ActorFailure::Closed, error.0.command));
        }
        Ok(RequestTicket {
            id,
            inbox: self.inbox.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopKind {
    Graceful,
    Requested,
    TransportFailed,
    InstanceFailed,
    AccountingFailed,
}
#[derive(Debug)]
pub struct ActorExit {
    pub kind: StopKind,
    pub exit_status: Option<ExitStatus>,
    pub containment_error: Option<io::Error>,
    pub reap_error: Option<io::Error>,
    pub shutdown_failure: Option<ProcessError>,
}
pub struct ActorJoinError {
    pub kind: ActorFailure,
    pub task: Option<tokio::task::JoinError>,
}
impl fmt::Debug for ActorJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorJoinError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ActorJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}
impl std::error::Error for ActorJoinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.task
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}
/// The instance supervisor retains this owner and joins it before releasing
/// database ownership. Dropping it requests fatal containment, never completion.
pub struct TransportOwner {
    client: TransportClient,
    stop: watch::Sender<bool>,
    join: Option<JoinHandle<ActorExit>>,
    worker_process_id: Option<u32>,
}
impl TransportOwner {
    pub async fn start(
        executable: &Path,
        boot: RootBoot,
        process_limits: ProcessLimits,
        limits: ActorLimits,
    ) -> Result<(Self, ProcessSnapshot), StartError> {
        if !limits.valid(process_limits.max_frame_bytes) {
            return Err(StartError::Actor(ActorFailure::InvalidLimits));
        }
        tokio::runtime::Handle::try_current()
            .map_err(|_| StartError::Actor(ActorFailure::RuntimeUnavailable))?;
        let values = boot.bounds.values;
        let context_limit = boot.max_recipe_contexts;
        let deadlines = HostingDeadlines {
            startup_timeout: boot.startup_timeout,
            response_timeout: process_limits.response_timeout,
        };
        let (mut process, ready) = GlobalProcess::start(executable, boot, process_limits)
            .await
            .map_err(StartError::Worker)?;
        let context_capacity = match ready.recipe_context_capacity {
            Some(capacity) if capacity.limit == context_limit && capacity.active == 0 => capacity,
            _ => {
                process.terminate().await;
                return Err(StartError::Actor(ActorFailure::InvalidLimits));
            }
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (stop, stop_rx) = watch::channel(false);
        let (stopped, observed_stop) = watch::channel(None);
        let worker_process_id = process.worker_process_id();
        let (frame_policy, observed_frame) = watch::channel(FrameRequirements {
            configured_frame_bytes: process_limits.max_frame_bytes,
            max_retained_frame_bytes: 0,
        });
        let inbox = CompletionInbox {
            ledger: Arc::new(Mutex::new(Ledger {
                values,
                limits,
                deadlines,
                frame: frame_policy,
                retained_frames: BTreeMap::new(),
                records: BTreeMap::new(),
                ordinary: Credits::default(),
                control: Credits::default(),
                closed: false,
            })),
        };
        let (values_tx, values_rx) = watch::channel(values);
        let (contexts_tx, contexts_rx) = watch::channel(context_capacity);
        let (memory_tx, memory_rx) = watch::channel(ready.worker_memory());
        let client = TransportClient {
            tx,
            control_tx,
            inbox: inbox.clone(),
            values: values_rx,
            recipe_contexts: contexts_rx,
            memory: memory_rx,
            frame: observed_frame,
            stopped: observed_stop,
        };
        let join = tokio::spawn(run(
            process,
            rx,
            control_rx,
            stop_rx,
            inbox,
            stopped,
            RuntimeObservations {
                values: values_tx,
                recipe_contexts: contexts_tx,
                memory: memory_tx,
            },
        ));
        Ok((
            Self {
                client,
                stop,
                join: Some(join),
                worker_process_id,
            },
            ready,
        ))
    }
    pub fn client(&self) -> TransportClient {
        self.client.clone()
    }
    /// Spawned process identity for trusted host diagnostics. Retained after
    /// exit; it is not proof of liveness or permission to signal another process.
    pub fn worker_process_id(&self) -> Option<u32> {
        self.worker_process_id
    }
    pub fn request_termination(&self) {
        self.stop.send_replace(true);
    }
    pub async fn join(&mut self) -> Result<ActorExit, ActorJoinError> {
        // Keep the handle if this wait is cancelled; a later wait can still
        // observe actual containment acknowledgement.
        let result = self
            .join
            .as_mut()
            .ok_or(ActorJoinError {
                kind: ActorFailure::Closed,
                task: None,
            })?
            .await;
        drop(self.join.take());
        result.map_err(|error| ActorJoinError {
            kind: ActorFailure::Join,
            task: Some(error),
        })
    }
}
impl Drop for TransportOwner {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
struct RuntimeObservations {
    values: watch::Sender<VmBounds>,
    recipe_contexts: watch::Sender<crate::process::RecipeContextCapacity>,
    memory: watch::Sender<crate::heap::WorkerMemoryStatus>,
}

async fn run(
    mut process: GlobalProcess,
    mut rx: mpsc::UnboundedReceiver<Envelope>,
    mut control_rx: mpsc::UnboundedReceiver<Envelope>,
    mut stop: watch::Receiver<bool>,
    inbox: CompletionInbox,
    stopped: watch::Sender<Option<StopKind>>,
    observed: RuntimeObservations,
) -> ActorExit {
    let mut shutdown_failure = None;
    let mut control_burst = 0usize;
    let subscription = inbox
        .ledger
        .lock()
        .map(|ledger| ledger.frame.subscribe())
        .map_err(|_| ());
    let mut frame_changes = match subscription {
        Ok(receiver) => receiver,
        Err(_) => {
            let exit_status = process.terminate().await;
            stopped.send_replace(Some(StopKind::AccountingFailed));
            return ActorExit {
                kind: StopKind::AccountingFailed,
                exit_status,
                containment_error: process.take_containment_error(),
                reap_error: process.take_reap_error(),
                shutdown_failure: None,
            };
        }
    };
    let (kind, exit_status) = loop {
        let deadline = inbox
            .ledger
            .lock()
            .map(|ledger| ledger.deadlines.response_timeout)
            .map_err(|_| ());
        let Ok(deadline) = deadline else {
            break (StopKind::AccountingFailed, process.terminate().await);
        };
        process.set_response_timeout(deadline);
        let next = tokio::select! { biased;
            observed = process.wait_idle_exit() => {
                let status = if observed.is_some() { observed } else { process.terminate().await };
                let mut failure = ProcessError::new(ProcessFailure::Transport);
                failure.exit_status = status;
                shutdown_failure = Some(failure);
                break (StopKind::TransportFailed, status);
            }
            changed = frame_changes.changed() => {
                if changed.is_err() {
                    break (StopKind::AccountingFailed, process.terminate().await);
                }
                let requirements = *frame_changes.borrow_and_update();
                let required = requirements.required_capacity();
                let frame = process.frame_status();
                if required < frame.required_capacity_bytes {
                    let Some(settings) = process.task_settings() else {
                        break (StopKind::AccountingFailed, process.terminate().await);
                    };
                    // This owner-only mechanical RPC is bounded by the current
                    // acknowledged frame/deadline. It neither consumes a caller
                    // ticket nor advances Python. New accepts cannot increase
                    // this requirement above their configured bound.
                    let result = process.exchange(WorkerCommand::ReconcileFrameCapacity {
                        expected_settings_revision: settings.revision,
                        configured_frame_bytes: requirements.configured_frame_bytes,
                        required_capacity_bytes: required,
                    }).await;
                    match result {
                        Ok(snapshot) if snapshot.boundary.is_none()
                            && snapshot.recipe.is_none() && snapshot.admitted_task.is_none()
                            && snapshot.stdout.is_empty() && snapshot.withheld_answers.is_empty() => {
                            observed.memory.send_replace(snapshot.worker_memory());
                        }
                        result => {
                            let failure = match result {
                                Err(error) => error,
                                Ok(snapshot) => {
                                    let mut error = ProcessError::new(ProcessFailure::Protocol);
                                    error.snapshot = Some(Box::new(snapshot)); error
                                }
                            };
                            shutdown_failure = Some(failure);
                            break (StopKind::TransportFailed, process.terminate().await);
                        }
                    }
                }
                continue;
            }
            next = next_command(&mut rx, &mut control_rx, &mut stop, control_burst >= 8) => next,
        };
        let Some((envelope, control)) = next else {
            break (StopKind::Requested, process.terminate().await);
        };
        control_burst = if control {
            control_burst.saturating_add(1)
        } else {
            0
        };
        let begun = inbox.ledger.lock().map_err(|_| ()).and_then(|mut ledger| {
            let record = ledger.records.get_mut(&envelope.id).ok_or(())?;
            record.transport_started = true;
            Ok(())
        });
        if begun.is_err() {
            break (StopKind::AccountingFailed, process.terminate().await);
        }
        process.set_response_timeout(envelope.response_timeout);
        let transactional = envelope.heap_commit.is_some() || envelope.durable_settings.is_some();
        let mut outcome = {
            let mut exchange = RetainedExchange {
                process: &mut process,
                max_frame_bytes: envelope.max_frame_bytes,
            };
            if let Some(commit) = envelope.heap_commit {
                heap_transaction(&mut exchange, envelope.command, commit).await
            } else if let Some(durable) = envelope.durable_settings {
                runtime_transaction(&mut exchange, envelope.command, durable).await
            } else {
                exchange.exchange(envelope.command).await
            }
        };
        // The private ledger already holds the exact original command. Do not
        // retain a second decoded copy on a failed exchange.
        if let Err(error) = &mut outcome
            && !transactional
        {
            error.command = None;
        }
        if let Some(commit) = envelope.runtime_commit
            && let Ok(snapshot) = &outcome
        {
            // No later queued RPC, observation or successful receipt may escape
            // between the worker's ACK and Rust publication. Expected worker
            // denials never invoke this callback or change Rust policy.
            let failed = match std::panic::catch_unwind(AssertUnwindSafe(|| commit(snapshot))) {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(format!("Rust runtime publication failed: {error:?}")),
                Err(_) => Some("Rust runtime publication panicked".into()),
            };
            if let Some(diagnostic) = failed {
                let Ok(snapshot) = outcome else {
                    unreachable!("publication only follows a worker ACK")
                };
                let mut error = ProcessError::new(ProcessFailure::Protocol);
                error.snapshot = Some(Box::new(snapshot));
                error.diagnostic = Some(diagnostic);
                outcome = Err(error);
            }
        }
        if let Ok(snapshot) = &outcome {
            let publication = inbox
                .ledger
                .lock()
                .map_err(|_| ActorFailure::AccountingUnavailable)
                .and_then(|mut ledger| {
                    let retained = ledger
                        .retained_frames
                        .last_key_value()
                        .map_or(0, |(bound, _)| *bound);
                    if !ledger.limits.valid(snapshot.frame.configured_frame_bytes)
                        || snapshot.frame.effective_capacity_bytes < retained
                    {
                        return Err(ActorFailure::InvalidLimits);
                    }
                    let max_retained_frame_bytes = retained;
                    let next = FrameRequirements {
                        configured_frame_bytes: snapshot.frame.configured_frame_bytes,
                        max_retained_frame_bytes,
                    };
                    let before = *ledger.frame.borrow();
                    if before != next {
                        ledger.frame.send_replace(next);
                    }
                    observed.memory.send_replace(snapshot.worker_memory());
                    if let Some(values) = snapshot.vm_bounds {
                        ledger.values = values;
                    }
                    Ok(())
                });
            if let Err(kind) = publication {
                let Ok(snapshot) = outcome else {
                    unreachable!("publication follows ACK")
                };
                let mut error = ProcessError::new(ProcessFailure::Protocol);
                error.snapshot = Some(Box::new(snapshot));
                error.diagnostic = Some(format!("frame publication failed: {kind:?}"));
                outcome = Err(error);
            }
        }
        if let Ok(snapshot) = &outcome
            && let Some(bounds) = snapshot.vm_bounds
        {
            observed.values.send_replace(bounds);
        }
        if let Ok(snapshot) = &outcome
            && let Some(capacity) = snapshot.recipe_context_capacity
        {
            observed.recipe_contexts.send_replace(capacity);
        }
        let graceful = outcome
            .as_ref()
            .is_ok_and(|snapshot| snapshot.lifecycle == Lifecycle::Stopped);
        let instance_failed = match &outcome {
            Ok(snapshot) => snapshot.lifecycle == Lifecycle::Failed,
            Err(error) => error
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.lifecycle == Lifecycle::Failed),
        };
        let fatal = outcome.as_ref().is_err_and(|error| {
            matches!(
                error.kind,
                ProcessFailure::Transport
                    | ProcessFailure::Protocol
                    | ProcessFailure::Deadline
                    | ProcessFailure::Terminal
            )
        });
        let observed_exit = outcome.as_ref().err().and_then(|error| error.exit_status);
        let published = inbox.ledger.lock().map_err(|_| ()).and_then(|mut ledger| {
            let record = ledger.records.get_mut(&envelope.id).ok_or(())?;
            record.outcome = Some(outcome);
            record.ready.send_replace(true);
            Ok(())
        });
        if published.is_err() {
            break (StopKind::AccountingFailed, process.terminate().await);
        }
        if graceful {
            break match process.wait_stopped().await {
                Ok(status) => (StopKind::Graceful, Some(status)),
                Err(error) => {
                    let status = error.exit_status;
                    shutdown_failure = Some(error);
                    (StopKind::TransportFailed, status)
                }
            };
        }
        if fatal || instance_failed {
            break (
                if instance_failed {
                    StopKind::InstanceFailed
                } else {
                    StopKind::TransportFailed
                },
                if observed_exit.is_some() {
                    observed_exit
                } else {
                    process.terminate().await
                },
            );
        }
    };
    rx.close();
    control_rx.close();
    if let Ok(mut ledger) = inbox.ledger.lock() {
        ledger.closed = true;
        for record in ledger
            .records
            .values_mut()
            .filter(|record| record.outcome.is_none())
        {
            record.outcome = Some(Err(ProcessError::new(ProcessFailure::Terminal)));
            record.ready.send_replace(true);
        }
    }
    stopped.send_replace(Some(kind));
    ActorExit {
        kind,
        exit_status,
        containment_error: process.take_containment_error(),
        reap_error: process.take_reap_error(),
        shutdown_failure,
    }
}

/// The sole owner holds every subsequent RPC while actual feasibility, durable
/// outcome and publication settle. No provisional policy or Python execution
/// occurs before the durable commit; a rejected write needs no rollback.
async fn runtime_transaction(
    process: &mut RetainedExchange<'_>,
    command: WorkerCommand,
    durable: DurableSettingsCommit,
) -> ExchangeResult {
    let (expected, settings, values, contexts, reserve, heap_update, frame_update) = match &command
    {
        WorkerCommand::Recipe {
            command:
                RecipeCommand::UpdateRuntimeSettings {
                    expected_revision,
                    settings,
                    values,
                    max_recipe_contexts,
                    adapter_reserve_bytes,
                    heap_update,
                    frame_update,
                },
        } => (
            *expected_revision,
            *settings,
            Some(*values),
            *max_recipe_contexts,
            *adapter_reserve_bytes,
            *heap_update,
            *frame_update,
        ),
        WorkerCommand::Recipe {
            command:
                RecipeCommand::UpdateSettings {
                    expected_revision,
                    settings,
                },
        } => (*expected_revision, *settings, None, None, None, None, None),
        _ => return Err(ProcessError::new(ProcessFailure::Protocol)),
    };
    // Child commands can have updated the heap since the service's last root
    // snapshot. Observe the actual current layout inside this same owner fence.
    let before = process.exchange(WorkerCommand::Inspect).await?;
    if before.boundary.is_some()
        || before.admitted_task.is_some()
        || before.recipe.is_some()
        || !before.stdout.is_empty()
        || !before.withheld_answers.is_empty()
        || before.heap.effective.is_none()
        || before.vm_bounds.is_none()
    {
        let mut error = ProcessError::new(ProcessFailure::Protocol);
        error.snapshot = Some(Box::new(before));
        return Err(error);
    }
    let values = values
        .or(before.vm_bounds)
        .ok_or_else(|| ProcessError::new(ProcessFailure::Protocol))?;
    let probe = process
        .exchange(WorkerCommand::ValidateRuntimeSettings {
            candidate: crate::process::RuntimeSettingsCandidate {
                expected_revision: expected,
                settings,
                values,
                max_recipe_contexts: contexts,
                adapter_reserve_bytes: reserve,
                heap_update,
                frame_update,
            },
        })
        .await?;
    if probe.boundary.is_some()
        || probe.admitted_task.is_some()
        || probe.recipe.is_some()
        || !probe.stdout.is_empty()
        || !probe.withheld_answers.is_empty()
        || probe.runtime_settings() != before.runtime_settings()
        || probe.heap != before.heap
    {
        let mut error = ProcessError::new(ProcessFailure::Protocol);
        error.snapshot = Some(Box::new(probe));
        return Err(error);
    }
    match tokio::time::timeout(
        durable.timeout,
        AssertUnwindSafe(durable.future).catch_unwind(),
    )
    .await
    {
        Ok(Ok(HeapCommitOutcome::Committed)) => {
            // A failure now follows a confirmed durable mutation. Preserve the
            // actual worker receipt and fence; it is never a harmless denial.
            process.exchange(command).await.map_err(|mut error| {
                error.kind = ProcessFailure::Transport;
                if error.snapshot.is_none() {
                    // Last observation only: it cannot establish whether the
                    // subsequent publication reached the worker.
                    error.snapshot = Some(Box::new(probe));
                }
                if error.diagnostic.is_none() {
                    error.diagnostic = Some(
                        "durable settings committed; worker publication requires reconciliation"
                            .into(),
                    );
                }
                error
            })
        }
        Ok(Ok(HeapCommitOutcome::Rejected)) => {
            let mut error = ProcessError::new(ProcessFailure::Vm(
                crate::VmFailure::SettingsRevisionConflict,
            ));
            error.snapshot = Some(Box::new(probe));
            Err(error)
        }
        _ => {
            let mut error = ProcessError::new(ProcessFailure::Transport);
            error.snapshot = Some(Box::new(probe));
            error.diagnostic = Some("durable settings outcome requires reconciliation".into());
            Err(error)
        }
    }
}

// This barrier is inside the sole transport owner. Direct child-context
// commands remain queued too; holding only the service loop would be insufficient.
async fn heap_transaction(
    process: &mut RetainedExchange<'_>,
    command: WorkerCommand,
    commit: HeapCommit,
) -> ExchangeResult {
    let WorkerCommand::UpdateHeap {
        expected_revision,
        settings,
        automatic: false,
    } = command
    else {
        return Err(ProcessError::new(ProcessFailure::Protocol));
    };
    let before = process.exchange(WorkerCommand::Inspect).await?;
    let old = before
        .heap
        .effective
        .ok_or_else(|| ProcessError::new(ProcessFailure::Protocol))?;
    let provisional = process
        .exchange(WorkerCommand::UpdateHeap {
            expected_revision,
            settings,
            automatic: false,
        })
        .await?;
    if provisional.heap.effective != Some(settings) || provisional.heap.pending_reduction {
        let mut error = ProcessError::new(ProcessFailure::Protocol);
        error.snapshot = Some(Box::new(provisional));
        return Err(error);
    }
    match tokio::time::timeout(
        commit.timeout,
        AssertUnwindSafe(commit.future).catch_unwind(),
    )
    .await
    {
        Ok(Ok(HeapCommitOutcome::Committed)) => Ok(provisional),
        Ok(Ok(HeapCommitOutcome::Rejected)) => {
            let revision = settings
                .revision
                .checked_add(1)
                .ok_or_else(|| ProcessError::new(ProcessFailure::Protocol))?;
            let mut rollback = process
                .exchange(WorkerCommand::UpdateHeap {
                    expected_revision: settings.revision,
                    settings: HeapSettings {
                        revision,
                        max_vm_bytes: old.max_vm_bytes,
                    },
                    automatic: false,
                })
                .await
                .map_err(|mut error| {
                    // A failed rollback is fatal even when its underlying limit
                    // error would normally be recoverable before a DB mutation.
                    error.kind = ProcessFailure::Transport;
                    error
                })?;
            if rollback.heap.effective.is_none_or(|heap| {
                heap.revision != revision || heap.max_vm_bytes != old.max_vm_bytes
            }) {
                let mut error = ProcessError::new(ProcessFailure::Protocol);
                error.snapshot = Some(Box::new(rollback));
                return Err(error);
            }
            if before.heap.pending_reduction {
                // A rejected manual edit must also preserve an earlier
                // automatic target, not silently cancel its pending reduction.
                let pending = before
                    .heap
                    .desired
                    .ok_or_else(|| ProcessError::new(ProcessFailure::Protocol))?;
                let restored = HeapSettings {
                    revision: revision
                        .checked_add(1)
                        .ok_or_else(|| ProcessError::new(ProcessFailure::Protocol))?,
                    max_vm_bytes: pending.max_vm_bytes,
                };
                rollback = process
                    .exchange(WorkerCommand::UpdateHeap {
                        expected_revision: revision,
                        settings: restored,
                        automatic: true,
                    })
                    .await
                    .map_err(|mut error| {
                        error.kind = ProcessFailure::Transport;
                        error
                    })?;
                if rollback.heap.desired != Some(restored)
                    || (!rollback.heap.pending_reduction
                        && rollback.heap.effective != Some(restored))
                {
                    let mut error = ProcessError::new(ProcessFailure::Protocol);
                    error.snapshot = Some(Box::new(rollback));
                    return Err(error);
                }
            }
            let mut error = ProcessError::new(ProcessFailure::Vm(
                crate::VmFailure::SettingsRevisionConflict,
            ));
            error.snapshot = Some(Box::new(rollback));
            Err(error)
        }
        _ => {
            // A missing commit outcome cannot prove rollback is safe. Retain
            // the provisional state and contain before any next VM command.
            let mut error = ProcessError::new(ProcessFailure::Transport);
            error.snapshot = Some(Box::new(provisional));
            Err(error)
        }
    }
}

// Reserve control/completion capacity without starving admitted work. Both
// queues are bounded and instance termination has priority in every round.
async fn next_command(
    rx: &mut mpsc::UnboundedReceiver<Envelope>,
    control: &mut mpsc::UnboundedReceiver<Envelope>,
    stop: &mut watch::Receiver<bool>,
    prefer_work: bool,
) -> Option<(Envelope, bool)> {
    if prefer_work {
        tokio::select! { biased;
            _ = stop.changed() => None,
            envelope = rx.recv() => envelope.map(|envelope| (envelope, false)),
            envelope = control.recv() => envelope.map(|envelope| (envelope, true)),
        }
    } else {
        tokio::select! { biased;
            _ = stop.changed() => None,
            envelope = control.recv() => envelope.map(|envelope| (envelope, true)),
            envelope = rx.recv() => envelope.map(|envelope| (envelope, false)),
        }
    }
}
