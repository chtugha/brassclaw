//! Instance-owned IPC, independent of cancellable turn futures. This actor
//! serializes transport only; it never selects a step, dispatches a Tool,
//! retries work or resolves a Monty port autonomously.
use std::{
    collections::BTreeMap,
    fmt, io,
    path::Path,
    process::ExitStatus,
    sync::{Arc, Mutex},
};

use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    VmBounds,
    global::Lifecycle,
    process::{
        GlobalProcess, ProcessError, ProcessFailure, ProcessLimits, ProcessSnapshot, RecipeCommand,
        RootBoot, WorkerCommand, recover_command, retain_command,
    },
};

type ExchangeResult = Result<ProcessSnapshot, ProcessError>;

#[derive(Debug, Clone, Copy)]
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
        (1..=1024).contains(&self.max_unclaimed)
            && self.max_reserved_frame_bytes > frame
            && (1..=1024).contains(&self.max_control_unclaimed)
            && self.max_control_reserved_frame_bytes > frame
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
    records: BTreeMap<RequestId, Record>,
    ordinary: Credits,
    control: Credits,
    closed: bool,
}
impl Ledger {
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
        let mut record = ledger.records.remove(&id).expect("checked record");
        let credits = ledger.credits_mut(record.control);
        credits.reserved -= record.reserved;
        credits.count -= 1;
        Ok(TransportReceipt {
            id,
            transport_started: record.transport_started,
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
struct Envelope {
    id: RequestId,
    command: WorkerCommand,
}
#[derive(Clone)]
pub struct TransportClient {
    tx: mpsc::Sender<Envelope>,
    control_tx: mpsc::Sender<Envelope>,
    inbox: CompletionInbox,
    limits: ActorLimits,
    values: VmBounds,
    frame: usize,
    stopped: watch::Receiver<Option<StopKind>>,
}
impl TransportClient {
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
        let (count_limit, byte_limit) = self.limits.lane(control);
        let ledger = match self.inbox.ledger.lock() {
            Ok(ledger) => ledger,
            Err(_) => return Err(submit(ActorFailure::AccountingUnavailable, command)),
        };
        if ledger.closed {
            return Err(submit(ActorFailure::Closed, command));
        }
        if ledger.credits(control).count >= count_limit {
            return Err(submit(ActorFailure::Backpressure, command));
        }
        drop(ledger);
        let wire = match retain_command(&command, self.values, self.frame) {
            Ok(wire) => wire,
            Err(kind) => return Err(submit(ActorFailure::Transport(kind), command)),
        };
        let reserved = match wire.len().checked_add(self.frame) {
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
        if ledger.credits(control).count >= count_limit {
            return Err(submit(ActorFailure::Backpressure, command));
        }
        let total = match ledger.credits(control).reserved.checked_add(reserved) {
            Some(total) if total <= byte_limit => total,
            _ => return Err(submit(ActorFailure::Backpressure, command)),
        };
        let id = RequestId(Uuid::new_v4());
        if ledger.records.contains_key(&id) {
            return Err(submit(ActorFailure::AccountingUnavailable, command));
        }
        let (ready, _) = watch::channel(false);
        ledger.records.insert(
            id,
            Record {
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
        let tx = if control { &self.control_tx } else { &self.tx };
        if let Err(error) = tx.try_send(Envelope { id, command }) {
            ledger.records.remove(&id);
            ledger.credits_mut(control).reserved -= reserved;
            ledger.credits_mut(control).count -= 1;
            return Err(match error {
                mpsc::error::TrySendError::Closed(envelope) => {
                    submit(ActorFailure::Closed, envelope.command)
                }
                mpsc::error::TrySendError::Full(envelope) => {
                    submit(ActorFailure::Backpressure, envelope.command)
                }
            });
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
        let (process, ready) = GlobalProcess::start(executable, boot, process_limits)
            .await
            .map_err(StartError::Worker)?;
        let (tx, rx) = mpsc::channel(limits.max_unclaimed);
        let (control_tx, control_rx) = mpsc::channel(limits.max_control_unclaimed);
        let (stop, stop_rx) = watch::channel(false);
        let (stopped, observed_stop) = watch::channel(None);
        let worker_process_id = process.worker_process_id();
        let inbox = CompletionInbox {
            ledger: Arc::new(Mutex::new(Ledger {
                records: BTreeMap::new(),
                ordinary: Credits::default(),
                control: Credits::default(),
                closed: false,
            })),
        };
        let client = TransportClient {
            tx,
            control_tx,
            inbox: inbox.clone(),
            limits,
            values,
            frame: process_limits.max_frame_bytes,
            stopped: observed_stop,
        };
        let join = tokio::spawn(run(process, rx, control_rx, stop_rx, inbox, stopped));
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
async fn run(
    mut process: GlobalProcess,
    mut rx: mpsc::Receiver<Envelope>,
    mut control_rx: mpsc::Receiver<Envelope>,
    mut stop: watch::Receiver<bool>,
    inbox: CompletionInbox,
    stopped: watch::Sender<Option<StopKind>>,
) -> ActorExit {
    let mut shutdown_failure = None;
    let mut control_burst = 0usize;
    let (kind, exit_status) = loop {
        let next = tokio::select! { biased;
            observed = process.wait_idle_exit() => {
                let status = if observed.is_some() { observed } else { process.terminate().await };
                let mut failure = ProcessError::new(ProcessFailure::Transport);
                failure.exit_status = status;
                shutdown_failure = Some(failure);
                break (StopKind::TransportFailed, status);
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
        let mut outcome = process.exchange(envelope.command).await;
        // The private ledger already holds the exact original command. Do not
        // retain a second decoded copy on a failed exchange.
        if let Err(error) = &mut outcome {
            error.command = None;
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

// Reserve control/completion capacity without starving admitted work. Both
// queues are bounded and instance termination has priority in every round.
async fn next_command(
    rx: &mut mpsc::Receiver<Envelope>,
    control: &mut mpsc::Receiver<Envelope>,
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
