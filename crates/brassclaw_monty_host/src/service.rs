//! Instance-owned scheduling of mechanical VM boundaries and actual host futures.
//!
//! Python selects operations. This service never chooses a Recipe, builds a
//! prompt, loops over model responses, retries an effect or fabricates its result.
//! A dropped turn waiter fences its own ports, while the instance retains the
//! admitted work and every started future until its actual result is observed.

use std::{
    collections::{BTreeMap, VecDeque},
    panic::AssertUnwindSafe,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use brassclaw_resources::LiveMontyTaskSettings;
use futures::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, watch},
    task::JoinHandle,
};

use crate::{
    ContinuationKey,
    global::{Lifecycle, RootExecutionIdentity},
    heap::{HeapSettings, HeapStatus},
    process::{
        PortAnswer, ProcessBoundary, ProcessFailure, ProcessLimits, ProcessSnapshot, RecipeCommand,
        RecipeEvent, RootBoot, TaskHandle, TaskSettings, WorkerCommand,
    },
    transport_actor::{
        ActorExit, ActorLimits, StartError, TransportClient, TransportOwner, TransportReceipt,
    },
};

/// Opaque product identities are data. Claims and credentials have no field here.
#[derive(Serialize)]
pub struct TaskInput {
    pub conversation_id: String,
    pub message_id: String,
    pub turn_id: String,
    pub run_id: String,
    pub user_input: String,
    pub history: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutcome {
    Completed { reply_ref: String },
    Failed { reason_kind: String },
}

/// Safe classified host error, never arbitrary provider/Tool diagnostics.
#[derive(Debug, Clone)]
pub struct PortFailure {
    reason: String,
}
impl PortFailure {
    pub fn new(reason: &str) -> Result<Self, ServiceFailure> {
        if valid_reason(reason) {
            Ok(Self {
                reason: reason.to_owned(),
            })
        } else {
            Err(ServiceFailure::InvalidPortResult)
        }
    }
}
fn valid_reason(reason: &str) -> bool {
    !reason.is_empty()
        && reason.len() <= 64
        && reason
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_lowercase())
}

/// Trusted task owner. Each call performs exactly the requested host operation.
/// Child hosting may use the supplied transport to execute one selected feed;
/// it cannot select or change the root's next Recipe step.
pub trait TaskPorts: Send + Sync + 'static {
    fn call(
        self: Arc<Self>,
        task: TaskHandle,
        transport: TransportClient,
        name: String,
        args: Vec<Value>,
        kwargs: BTreeMap<String, Value>,
    ) -> BoxFuture<'static, Result<Value, PortFailure>>;

    /// Verify a completed reply against this exact task's actual published
    /// transcript. Failure outcomes must also retain actual effect evidence.
    /// This is lifecycle validation, not a replacement reply or task workflow.
    fn finish(
        self: Arc<Self>,
        outcome: TaskOutcome,
    ) -> BoxFuture<'static, Result<TaskOutcome, PortFailure>>;

    /// Synchronously close this attempt's host admission. Cannot reopen it.
    fn fence(&self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceFailure {
    InvalidInput,
    InvalidLimits,
    InvalidPortResult,
    Backpressure,
    Closed,
    Transport,
    Protocol,
    Deadline,
    Join,
    SettingsConflict,
    UnsafeHeapReduction,
}

struct TaskSettingsPublication {
    expected: u64,
    settings: TaskSettings,
    result: watch::Sender<Option<Result<Arc<SettingsReceipt>, ServiceFailure>>>,
}
struct HeapPublication {
    expected: u64,
    settings: HeapSettings,
    automatic: bool,
    result: watch::Sender<Option<Result<HeapObservation, ServiceFailure>>>,
}
enum SettingsPublication {
    Task(TaskSettingsPublication),
    Heap(HeapPublication),
}
impl SettingsPublication {
    fn reject(self, failure: ServiceFailure) {
        match self {
            Self::Task(publication) => {
                publication.result.send_replace(Some(Err(failure)));
            }
            Self::Heap(publication) => {
                publication.result.send_replace(Some(Err(failure)));
            }
        }
    }
}

/// Last actual worker observation. A retained observation is neither a running
/// lifecycle claim nor an acknowledgement of external-effect settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapObservation {
    pub status: HeapStatus,
    pub vm_live_bytes: usize,
}
impl From<&ProcessSnapshot> for HeapObservation {
    fn from(snapshot: &ProcessSnapshot) -> Self {
        Self {
            status: snapshot.heap,
            vm_live_bytes: snapshot.vm_live_bytes,
        }
    }
}
pub struct SettingsReceipt {
    pub effective_settings: TaskSettings,
    pub accounting: Vec<crate::process::TaskAccounting>,
}
struct SettingsInbox {
    requests: mpsc::Receiver<SettingsPublication>,
    live: LiveMontyTaskSettings,
    heap: watch::Sender<HeapObservation>,
}

struct Control {
    cancelled: AtomicBool,
    changed: Arc<Notify>,
    ports: Arc<dyn TaskPorts>,
    result: watch::Sender<Option<Result<Arc<TaskReceipt>, ServiceFailure>>>,
}
/// Actual task completion plus any root answers withheld after fencing.
/// Payloads stay private to the trusted caller, never ordinary Python state.
pub struct TaskReceipt {
    /// The instance root that owned this queue/attempt. accounting=None still
    /// means this particular task never entered the interpreter.
    pub root: RootExecutionIdentity,
    pub outcome: TaskOutcome,
    pub withheld: Vec<crate::global::WithheldHostAnswer>,
    /// The worker's actual shared root/child account. None only when a queued
    /// task was cancelled before admission; never interpret it as zero usage.
    pub accounting: Option<crate::process::TaskAccounting>,
}
#[derive(Clone)]
pub struct TaskControl(Arc<Control>);
impl TaskControl {
    /// Nonblocking observation of the actual service receipt. No synthetic
    /// settlement is inferred from cancellation or a dropped turn waiter.
    pub fn receipt(&self) -> Option<Result<Arc<TaskReceipt>, ServiceFailure>> {
        self.0.result.borrow().clone()
    }

    pub fn cancel(&self) {
        self.0.ports.fence();
        self.0.cancelled.store(true, Ordering::Release);
        self.0.changed.notify_one();
    }

    /// Acknowledges local VM and host-future settlement only. External effects
    /// still use the task host's durable records; timeout never authorizes replay.
    pub async fn stopped(&self, timeout: Duration) -> Result<(), ServiceFailure> {
        tokio::time::timeout(timeout, self.wait())
            .await
            .map_err(|_| ServiceFailure::Deadline)??;
        Ok(())
    }

    pub async fn wait(&self) -> Result<Arc<TaskReceipt>, ServiceFailure> {
        let mut result = self.0.result.subscribe();
        loop {
            if let Some(outcome) = result.borrow_and_update().clone() {
                return outcome;
            }
            result.changed().await.map_err(|_| ServiceFailure::Closed)?;
        }
    }
}

pub struct TaskTicket {
    control: TaskControl,
}
impl TaskTicket {
    pub fn control(&self) -> TaskControl {
        self.control.clone()
    }
    pub async fn wait(&self) -> Result<Arc<TaskReceipt>, ServiceFailure> {
        self.control.wait().await
    }
}
impl Drop for TaskTicket {
    fn drop(&mut self) {
        // Also safe after completion. A later attempt has a different port owner.
        self.control.cancel();
    }
}

struct Admission {
    input: Value,
    control: TaskControl,
    // Covers the channel, the local FIFO and an uncertain Admit exchange.
    // Draining the channel must not increase the configured queue capacity.
    _credit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub struct ServiceClient {
    admissions: mpsc::Sender<Admission>,
    admission_credits: Arc<Semaphore>,
    changed: Arc<Notify>,
    closed: Arc<AtomicBool>,
    values: crate::VmBounds,
    frame: usize,
    settings: mpsc::Sender<SettingsPublication>,
    effective_settings: LiveMontyTaskSettings,
    observed_heap: watch::Receiver<HeapObservation>,
}
impl ServiceClient {
    /// Fence new admissions synchronously. The retained ServiceOwner still
    /// performs shutdown/cancellation and observes actual started host futures.
    pub fn close_admission(&self) {
        self.closed.store(true, Ordering::Release);
        self.admission_credits.close();
        self.changed.notify_one();
    }

    /// Read-only by convention: all trusted settings writers must use
    /// publish_settings so VM acknowledgement precedes Rust publication.
    pub fn live_task_settings(&self) -> LiveMontyTaskSettings {
        self.effective_settings.clone()
    }

    /// Dropping the waiter does not cancel an accepted settings publication.
    /// The instance retains the command; current effective state is readable
    /// through live_task_settings even if its caller abandons the receipt.
    pub async fn publish_settings(
        &self,
        expected: u64,
        settings: TaskSettings,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        let (result, mut receipt) = watch::channel(None);
        self.settings
            .try_send(SettingsPublication::Task(TaskSettingsPublication {
                expected,
                settings,
                result,
            }))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => ServiceFailure::Backpressure,
                mpsc::error::TrySendError::Closed(_) => ServiceFailure::Closed,
            })?;
        self.changed.notify_one();
        loop {
            if let Some(result) = receipt.borrow_and_update().clone() {
                return result;
            }
            receipt
                .changed()
                .await
                .map_err(|_| ServiceFailure::Closed)?;
        }
    }

    pub fn heap_observation(&self) -> HeapObservation {
        *self.observed_heap.borrow()
    }

    /// Owned publication on the same bounded control lane as task settings.
    /// The worker decides feasibility against real current allocations, without
    /// an inspect/update race. A dropped caller never withdraws the request.
    pub async fn publish_heap(
        &self,
        expected: u64,
        settings: HeapSettings,
        automatic: bool,
    ) -> Result<HeapObservation, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        let (result, mut receipt) = watch::channel(None);
        self.settings
            .try_send(SettingsPublication::Heap(HeapPublication {
                expected,
                settings,
                automatic,
                result,
            }))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => ServiceFailure::Backpressure,
                mpsc::error::TrySendError::Closed(_) => ServiceFailure::Closed,
            })?;
        self.changed.notify_one();
        loop {
            if let Some(result) = *receipt.borrow_and_update() {
                return result;
            }
            receipt
                .changed()
                .await
                .map_err(|_| ServiceFailure::Closed)?;
        }
    }

    pub fn submit(
        &self,
        input: TaskInput,
        ports: Arc<dyn TaskPorts>,
    ) -> Result<TaskTicket, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        if self.heap_observation().status.pending_reduction {
            return Err(ServiceFailure::Backpressure);
        }
        let credit =
            self.admission_credits
                .clone()
                .try_acquire_owned()
                .map_err(|error| match error {
                    tokio::sync::TryAcquireError::Closed => ServiceFailure::Closed,
                    tokio::sync::TryAcquireError::NoPermits => ServiceFailure::Backpressure,
                })?;
        if [
            &input.conversation_id,
            &input.message_id,
            &input.turn_id,
            &input.run_id,
        ]
        .iter()
        .any(|id| id.is_empty())
        {
            return Err(ServiceFailure::InvalidInput);
        }
        if input.history.len() > self.values.max_value_nodes
            || [
                &input.conversation_id,
                &input.message_id,
                &input.turn_id,
                &input.run_id,
                &input.user_input,
            ]
            .iter()
            .any(|value| value.len() > self.values.max_value_bytes)
            || input
                .history
                .iter()
                .any(|value| !crate::process::transport_value(value, self.values, self.frame))
        {
            return Err(ServiceFailure::InvalidInput);
        }
        let input = serde_json::to_value(input).map_err(|_| ServiceFailure::InvalidInput)?;
        // Reserve the worker-added task token before admission. Serialization
        // uses the largest possible continuation ordinal and a fixed-size UUID.
        let mut values = self.values;
        values.max_value_nodes = values.max_value_nodes.saturating_sub(2);
        values.max_value_bytes = values.max_value_bytes.saturating_sub(64);
        let probe = WorkerCommand::Admit {
            key: ContinuationKey {
                vm_id: uuid::Uuid::nil(),
                ordinal: u64::MAX,
            },
            task: input.clone(),
        };
        crate::process::retain_command(&probe, values, self.frame)
            .map_err(|_| ServiceFailure::InvalidInput)?;
        let (result, _) = watch::channel(None);
        let control = TaskControl(Arc::new(Control {
            cancelled: AtomicBool::new(false),
            changed: Arc::clone(&self.changed),
            ports,
            result,
        }));
        self.admissions
            .try_send(Admission {
                input,
                control: control.clone(),
                _credit: credit,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => ServiceFailure::Backpressure,
                mpsc::error::TrySendError::Closed(_) => ServiceFailure::Closed,
            })?;
        Ok(TaskTicket { control })
    }
}

/// Retained actual results on instance failure. Contents deliberately have no
/// Debug/serialization. Receiving this object grants no retry or acknowledgement.
pub struct TaskEvidence {
    pub task: TaskHandle,
    pub ports: Arc<dyn TaskPorts>,
    pub host_results: Vec<Value>,
    pub reported_outcome: Option<TaskOutcome>,
    pub withheld: Vec<crate::global::WithheldHostAnswer>,
}
pub struct AdmissionEvidence {
    pub input: Value,
    pub ports: Arc<dyn TaskPorts>,
}
pub struct ServiceExit {
    pub root: RootExecutionIdentity,
    pub failure: Option<ServiceFailure>,
    pub transport: Result<ActorExit, ServiceFailure>,
    pub tasks: Vec<TaskEvidence>,
    pub pending_admission: Option<AdmissionEvidence>,
    /// Original work that never crossed an Admit boundary. Failure is explicit;
    /// retaining it does not authorize replay or acknowledge durable settlement.
    pub queued_admissions: Vec<AdmissionEvidence>,
    pub failed_exchanges: Vec<TransportReceipt>,
    pub rejected_commands: Vec<WorkerCommand>,
    pub transport_inbox: crate::transport_actor::CompletionInbox,
    pub last_snapshot: ProcessSnapshot,
}

pub struct ServiceOwner {
    client: ServiceClient,
    shutdown: watch::Sender<bool>,
    join: Option<JoinHandle<ServiceExit>>,
    worker_process_id: Option<u32>,
}
impl ServiceOwner {
    pub async fn start(
        executable: &Path,
        boot: RootBoot,
        process: ProcessLimits,
        actor: ActorLimits,
        queue_capacity: usize,
    ) -> Result<Self, StartError> {
        let live = LiveMontyTaskSettings::new(boot.task_settings.into())
            .map_err(|_| StartError::Actor(crate::transport_actor::ActorFailure::InvalidLimits))?;
        Self::start_with_live_settings(executable, boot, process, actor, queue_capacity, live).await
    }

    pub async fn start_with_live_settings(
        executable: &Path,
        boot: RootBoot,
        process: ProcessLimits,
        actor: ActorLimits,
        queue_capacity: usize,
        live: LiveMontyTaskSettings,
    ) -> Result<Self, StartError> {
        if TaskSettings::from(live.current()) != boot.task_settings {
            return Err(StartError::Actor(
                crate::transport_actor::ActorFailure::InvalidLimits,
            ));
        }
        boot.heap_settings.ok_or(StartError::Actor(
            crate::transport_actor::ActorFailure::InvalidLimits,
        ))?;
        if !(1..=1024).contains(&queue_capacity) {
            return Err(StartError::Actor(
                crate::transport_actor::ActorFailure::InvalidLimits,
            ));
        }
        let workers = boot.bounds.workers;
        let values = boot.bounds.values;
        let (owner, ready) = TransportOwner::start(executable, boot, process, actor).await?;
        let worker_process_id = owner.worker_process_id();
        let root = ready.root.expect("transport verified root identity");
        // start_ready proves every live worker is parked before returning.
        let (admissions, rx) = mpsc::channel(queue_capacity);
        let admission_credits = Arc::new(Semaphore::new(queue_capacity));
        let (settings, settings_rx) = mpsc::channel(8);
        let (heap, observed_heap) = watch::channel(HeapObservation::from(&ready));
        let (shutdown, stop) = watch::channel(false);
        let changed = Arc::new(Notify::new());
        let closed = Arc::new(AtomicBool::new(false));
        let client = ServiceClient {
            admissions,
            admission_credits: admission_credits.clone(),
            changed: changed.clone(),
            closed: closed.clone(),
            values,
            frame: process.max_frame_bytes,
            settings,
            effective_settings: live.clone(),
            observed_heap,
        };
        let join = tokio::spawn(run(
            owner,
            ready,
            rx,
            stop,
            changed,
            closed,
            ShutdownBounds {
                root,
                admission_credits,
                workers,
                boundary_timeout: process.response_timeout,
                settings: SettingsInbox {
                    requests: settings_rx,
                    live,
                    heap,
                },
            },
        ));
        Ok(Self {
            client,
            shutdown,
            join: Some(join),
            worker_process_id,
        })
    }
    pub fn client(&self) -> ServiceClient {
        self.client.clone()
    }
    /// Retained spawned process identity for trusted host diagnostics only.
    /// This is not a liveness observation or a task-visible capability.
    pub fn worker_process_id(&self) -> Option<u32> {
        self.worker_process_id
    }
    pub fn request_shutdown(&self) {
        self.client.close_admission();
        self.shutdown.send_replace(true);
    }
    pub async fn join(&mut self) -> Result<ServiceExit, ServiceFailure> {
        let result = self.join.as_mut().ok_or(ServiceFailure::Closed)?.await;
        self.join.take();
        result.map_err(|_| ServiceFailure::Join)
    }
}
impl Drop for ServiceOwner {
    fn drop(&mut self) {
        self.request_shutdown();
    }
}

struct TaskRecord {
    worker: u32,
    control: TaskControl,
    cancellation_sent: bool,
    outcome: Option<TaskOutcome>,
    retained: Vec<Value>,
    withheld: Vec<crate::global::WithheldHostAnswer>,
}
struct PortResult {
    task: TaskHandle,
    key: ContinuationKey,
    result: Result<Value, PortFailure>,
    finished: Option<Result<TaskOutcome, PortFailure>>,
    panicked: bool,
}
type Calls = FuturesUnordered<BoxFuture<'static, PortResult>>;
struct ShutdownBounds {
    root: RootExecutionIdentity,
    admission_credits: Arc<Semaphore>,
    workers: u32,
    boundary_timeout: Duration,
    settings: SettingsInbox,
}

#[derive(Default)]
struct ExchangeEvidence {
    failed: Vec<TransportReceipt>,
    rejected: Vec<WorkerCommand>,
}
async fn exchange(
    client: &TransportClient,
    evidence: &mut ExchangeEvidence,
    command: WorkerCommand,
) -> Result<ProcessSnapshot, ServiceFailure> {
    let ticket = client.try_submit(command).map_err(|error| {
        evidence.rejected.push(*error.command);
        ServiceFailure::Transport
    })?;
    let receipt = ticket.wait().await.map_err(|_| ServiceFailure::Transport)?;
    if receipt.outcome.is_err() {
        evidence.failed.push(receipt);
        return Err(ServiceFailure::Transport);
    }
    // Preserve successful withheld answers in the owning task receipt below.
    receipt.outcome.map_err(|_| ServiceFailure::Transport)
}

/// An expected rejected heap edit executes no Python or host effect. Its real
/// receipt is consumed as a denial; transport/protocol failures retain evidence
/// and trigger containment exactly like every other failed exchange.
async fn update_heap(
    client: &TransportClient,
    evidence: &mut ExchangeEvidence,
    snapshot: &mut ProcessSnapshot,
    expected: u64,
    settings: HeapSettings,
    automatic: bool,
) -> Result<Option<ServiceFailure>, ServiceFailure> {
    let ticket = client
        .try_submit(WorkerCommand::UpdateHeap {
            expected_revision: expected,
            settings,
            automatic,
        })
        .map_err(|error| {
            evidence.rejected.push(*error.command);
            ServiceFailure::Transport
        })?;
    let receipt = ticket.wait().await.map_err(|_| ServiceFailure::Transport)?;
    let denial = match &receipt.outcome {
        Err(error) => match error.kind {
            ProcessFailure::Vm(crate::VmFailure::UnsafeHeapReduction) => {
                Some(ServiceFailure::UnsafeHeapReduction)
            }
            ProcessFailure::Vm(crate::VmFailure::SettingsRevisionConflict) => {
                Some(ServiceFailure::SettingsConflict)
            }
            ProcessFailure::Vm(crate::VmFailure::InvalidBounds) => {
                Some(ServiceFailure::InvalidLimits)
            }
            _ => None,
        },
        Ok(_) => None,
    };
    if receipt.outcome.is_err() && denial.is_none() {
        evidence.failed.push(receipt);
        return Err(ServiceFailure::Transport);
    }
    let observed = match &receipt.outcome {
        Ok(next) => Some(next),
        Err(error) => error.snapshot.as_deref(),
    };
    if observed.is_none_or(|next| {
        next.boundary.is_some()
            || next.recipe.is_some()
            || next.admitted_task.is_some()
            || !next.withheld_answers.is_empty()
            || (denial.is_none()
                && (next.heap.desired != Some(settings)
                    || (!automatic && next.heap.effective != Some(settings))))
    }) {
        // Preserve the full unexpected receipt, including actual VM state and
        // any withheld host answers. Never discard them while classifying IPC.
        evidence.failed.push(receipt);
        return Err(ServiceFailure::Protocol);
    }
    let mut next = match receipt.outcome {
        Ok(next) => next,
        Err(mut error) => *error.snapshot.take().ok_or(ServiceFailure::Protocol)?,
    };
    // Side commands never advance the root. Keep the original unconsumed
    // Python boundary while using the worker's actual resource observation.
    next.boundary = snapshot.boundary.take();
    *snapshot = next;
    Ok(denial)
}

async fn run(
    mut owner: TransportOwner,
    mut snapshot: ProcessSnapshot,
    mut admissions: mpsc::Receiver<Admission>,
    mut stop: watch::Receiver<bool>,
    changed: Arc<Notify>,
    closed: Arc<AtomicBool>,
    mut shutdown: ShutdownBounds,
) -> ServiceExit {
    let transport = owner.client();
    let mut tasks = BTreeMap::<TaskHandle, TaskRecord>::new();
    let mut calls = Calls::new();
    let mut shutting_down = false;
    let mut exchanges = ExchangeEvidence::default();
    let mut pending_admission: Option<Admission> = None;
    let mut queued = VecDeque::<Admission>::new();
    let mut queued_evidence = Vec::new();
    let mut heap_inspection: Option<tokio::time::Instant> = None;
    let result = async {
        loop {
            // These inputs have never entered the VM. Cancellation can therefore
            // acknowledge no VM account without waiting for a free worker or
            // an unrelated external host future. FIFO survivors retain order.
            queued.retain(|admission| {
                if !admission.control.0.cancelled.load(Ordering::Acquire) { return true; }
                admission.control.0.ports.fence();
                admission.control.0.result.send_replace(Some(Ok(Arc::new(TaskReceipt {
                    root: shutdown.root,
                    outcome: TaskOutcome::Failed { reason_kind: "task_cancelled".into() },
                    withheld: Vec::new(), accounting: None,
                }))));
                false
            });
            if !shutting_down && transport.stop_kind().map_err(|_| ServiceFailure::Transport)?.is_some() {
                return Err(ServiceFailure::Transport);
            }
            if *stop.borrow_and_update() && !shutting_down {
                shutting_down = true;
                closed.store(true, Ordering::Release);
                shutdown.admission_credits.close();
                admissions.close();
                for task in tasks.values() { task.control.cancel(); }
            }
            if shutting_down {
                while let Ok(admission) = admissions.try_recv() { queued.push_back(admission); }
                for admission in queued.drain(..) {
                    queued_evidence.push(reject(admission, ServiceFailure::Closed));
                }
            }
            if snapshot.heap.pending_reduction && !shutting_down {
                let now = tokio::time::Instant::now();
                let due = heap_inspection.get_or_insert(now + Duration::from_millis(100));
                if now >= *due {
                    let mut observed = exchange(&transport, &mut exchanges, WorkerCommand::Inspect).await?;
                    if observed.boundary.is_some() || observed.recipe.is_some() || observed.admitted_task.is_some()
                        || !observed.withheld_answers.is_empty() {
                        snapshot = observed;
                        return Err(ServiceFailure::Protocol);
                    }
                    observed.boundary = snapshot.boundary.take();
                    snapshot = observed;
                    heap_inspection = Some(tokio::time::Instant::now() + Duration::from_millis(100));
                }
            } else {
                heap_inspection = None;
            }
            // Give settings a bounded service turn even while the root keeps
            // yielding CPU slices. No host dispatch/admission occurs between
            // the worker's acknowledgement and publication to Rust consumers.
            shutdown.settings.heap.send_replace(HeapObservation::from(&snapshot));
            if let Ok(publication) = shutdown.settings.requests.try_recv() {
                if shutting_down {
                    publication.reject(ServiceFailure::Closed);
                    continue;
                }
                match publication {
                    SettingsPublication::Task(publication) => {
                        if publication.expected != shutdown.settings.live.current().revision
                            || publication.settings.revision <= publication.expected {
                            publication.result.send_replace(Some(Err(ServiceFailure::SettingsConflict)));
                        } else if LiveMontyTaskSettings::new(publication.settings.into()).is_err() {
                            publication.result.send_replace(Some(Err(ServiceFailure::InvalidLimits)));
                        } else {
                            let update = exchange(&transport, &mut exchanges, WorkerCommand::Recipe {
                                command: RecipeCommand::UpdateSettings {
                                    expected_revision: publication.expected, settings: publication.settings,
                                },
                            }).await;
                            let mut receipt = match update {
                                Ok(receipt) => receipt,
                                Err(error) => {
                                    publication.result.send_replace(Some(Err(error)));
                                    return Err(error);
                                }
                            };
                            if !matches!(receipt.recipe, Some(RecipeEvent::SettingsUpdated))
                                || receipt.effective_task_settings != Some(publication.settings)
                                || receipt.boundary.is_some() {
                                snapshot = receipt;
                                publication.result.send_replace(Some(Err(ServiceFailure::Protocol)));
                                return Err(ServiceFailure::Protocol);
                            }
                            // UpdateSettings does not advance the root. Preserve its
                            // actual unconsumed call/control boundary; replacing it
                            // with the side-command's None would strand execution.
                            receipt.boundary = snapshot.boundary.take();
                            snapshot = receipt;
                            if shutdown.settings.live.publish(publication.expected, publication.settings.into()).is_err() {
                                publication.result.send_replace(Some(Err(ServiceFailure::SettingsConflict)));
                                return Err(ServiceFailure::SettingsConflict);
                            }
                            publication.result.send_replace(Some(Ok(Arc::new(SettingsReceipt {
                                effective_settings: publication.settings,
                                accounting: snapshot.task_accounting.clone(),
                            }))));
                        }
                    }
                    SettingsPublication::Heap(publication) => {
                        let update = update_heap(&transport, &mut exchanges, &mut snapshot,
                            publication.expected, publication.settings, publication.automatic).await;
                        shutdown.settings.heap.send_replace(HeapObservation::from(&snapshot));
                        match update {
                            Ok(denial) => {
                                let result = match denial {
                                    Some(error) => Err(error),
                                    None => Ok(HeapObservation::from(&snapshot)),
                                };
                                publication.result.send_replace(Some(result));
                            }
                            Err(error) => {
                                publication.result.send_replace(Some(Err(error)));
                                return Err(error);
                            }
                        }
                    }
                }
                continue;
            }
            // A turn cancellation cannot cancel an IPC exchange or drop an
            // already-started host future. The root gets its addressed signal.
            for (id, task) in &mut tasks {
                if task.control.0.cancelled.load(Ordering::Acquire)
                    && !task.cancellation_sent && task.outcome.is_none() {
                    let receipt = exchange(&transport, &mut exchanges, WorkerCommand::Recipe {
                        command: RecipeCommand::CancelTask { task: *id },
                    }).await?;
                    if !matches!(receipt.recipe,
                        Some(RecipeEvent::CancellationRequested { task }) if task == *id) {
                        snapshot = receipt;
                        return Err(ServiceFailure::Protocol);
                    }
                    task.cancellation_sent = true;
                }
            }
            // The root has returned to this worker's real work wait only after
            // finish_task was resolved. Child release must report no pending host
            // correlation; otherwise preserve the evidence and fail containment.
            let releasable: Vec<_> = tasks.iter().filter_map(|(id, task)| {
                (task.outcome.is_some() && snapshot.work_waits.iter()
                    .any(|(worker, _)| *worker == task.worker)).then_some(*id)
            }).collect();
            for id in releasable {
                let accounting = snapshot.task_accounting.iter().find(|account| account.task == id)
                    .cloned().ok_or(ServiceFailure::Protocol)?;
                let mut receipt = exchange(&transport, &mut exchanges, WorkerCommand::Recipe {
                    command: RecipeCommand::CloseTask { task: id },
                }).await?;
                if !matches!(&receipt.recipe,
                    Some(RecipeEvent::Released { task: Some(task), contexts })
                    if *task == id && contexts.iter().all(|context| context.pending_host.is_none()))
                    || receipt.boundary.is_some() || receipt.admitted_task.is_some()
                    || !receipt.withheld_answers.is_empty() {
                    snapshot = receipt;
                    return Err(ServiceFailure::Protocol);
                }
                // CloseTask releases the actual child heaps without advancing
                // Python. Retain an unconsumed root boundary, but publish the
                // post-release heap/accounting before acknowledging completion.
                // An idle instance must not keep reporting freed task memory.
                receipt.boundary = snapshot.boundary.take();
                snapshot = receipt;
                shutdown.settings.heap.send_replace(HeapObservation::from(&snapshot));
                let task = tasks.remove(&id).expect("selected task");
                task.control.0.result.send_replace(Some(Ok(Arc::new(TaskReceipt {
                    root: shutdown.root,
                    outcome: task.outcome.expect("finished task"), withheld: task.withheld,
                    accounting: Some(accounting),
                }))));
            }

            match snapshot.boundary.take() {
                Some(ProcessBoundary::ControlYield { key }) => {
                    tokio::task::yield_now().await;
                    snapshot = exchange(&transport, &mut exchanges, WorkerCommand::ResumeControl { key }).await?;
                    continue;
                }
                Some(ProcessBoundary::HostCall { key, name, mut args, kwargs }) => {
                    if name == "await_next_task" {
                        if !snapshot.work_waits.iter().any(|(_, waiting)| *waiting == key) {
                            return Err(ServiceFailure::Protocol);
                        }
                    } else {
                        let task: TaskHandle = serde_json::from_value(
                            args.first().cloned().ok_or(ServiceFailure::Protocol)?)
                            .map_err(|_| ServiceFailure::Protocol)?;
                        let record = tasks.get(&task).ok_or(ServiceFailure::Protocol)?;
                        args.remove(0);
                        let ports = record.control.0.ports.clone();
                        let transport = transport.clone();
                        if name == "finish_task" {
                            if args.len() != 1 || !kwargs.is_empty() {
                                return Err(ServiceFailure::Protocol);
                            }
                            let outcome = parse_finish(args.remove(0))?;
                            calls.push(async move {
                                let expected = outcome.clone();
                                let finished = AssertUnwindSafe(async { ports.finish(outcome).await })
                                    .catch_unwind().await;
                                let panicked = finished.is_err();
                                let finished = finished.unwrap_or_else(|_| Err(PortFailure { reason: "host_panic".into() }))
                                    .and_then(|actual| if actual == expected { Ok(actual) }
                                        else { Err(PortFailure { reason: "task_completion_invalid".into() }) });
                                PortResult { task, key, result: Ok(Value::Null), finished: Some(finished), panicked }
                            }.boxed());
                        } else {
                            calls.push(async move {
                                let result = AssertUnwindSafe(async { ports.call(task, transport, name, args, kwargs).await })
                                    .catch_unwind().await;
                                let panicked = result.is_err();
                                let result = result.unwrap_or_else(|_| Err(PortFailure { reason: "host_panic".into() }));
                                PortResult { task, key, result, finished: None, panicked }
                            }.boxed());
                        }
                    }
                    // Defer this precise future to let unrelated root coroutines
                    // reach their boundaries; no polling or synthetic return.
                    snapshot = exchange(&transport, &mut exchanges, WorkerCommand::Defer { key }).await?;
                    continue;
                }
                Some(ProcessBoundary::Stopped) => {
                    if tasks.is_empty() && calls.is_empty() { return Ok(()); }
                    return Err(ServiceFailure::Protocol);
                }
                Some(ProcessBoundary::Waiting { .. }) | None => {}
            }

            if shutting_down && tasks.is_empty() && calls.is_empty() {
                // Pending admissions never ran. Their callers get an explicit
                // stopped result instead of an invented successful task.
                snapshot = exchange(&transport, &mut exchanges, WorkerCommand::BeginShutdown).await?;
                for _ in 0..shutdown.workers {
                    settle_shutdown_boundary(&transport, &mut exchanges, &mut snapshot,
                        shutdown.boundary_timeout).await?;
                    let key = snapshot.work_waits.first().ok_or(ServiceFailure::Protocol)?.1;
                    snapshot = exchange(&transport, &mut exchanges, WorkerCommand::CloseWorker { key }).await?;
                }
                settle_shutdown_boundary(&transport, &mut exchanges, &mut snapshot,
                    shutdown.boundary_timeout).await?;
                if snapshot.lifecycle != Lifecycle::Stopped { return Err(ServiceFailure::Protocol); }
                return Ok(());
            }

            let available = snapshot.work_waits.iter().find(|(worker, _)|
                !tasks.values().any(|task| task.worker == *worker)).copied();
            if !shutting_down && let Some((worker, key)) = available
                && let Some(admission) = queued.pop_front() {
                if snapshot.heap.pending_reduction {
                    // Known pre-VM rejection is returned to the original caller;
                    // do not accumulate these safe, unissued inputs indefinitely.
                    drop(reject(admission, ServiceFailure::Backpressure));
                    continue;
                }
                pending_admission = Some(admission);
                snapshot = exchange(&transport, &mut exchanges, WorkerCommand::Admit {
                    key, task: pending_admission.as_ref().expect("retained admission").input.clone(),
                }).await?;
                let task = snapshot.admitted_task.ok_or(ServiceFailure::Protocol)?;
                if tasks.contains_key(&task) { return Err(ServiceFailure::Protocol); }
                let admission = pending_admission.take().expect("retained admission");
                tasks.insert(task, TaskRecord { worker, control: admission.control,
                    cancellation_sent: false, outcome: None, retained: Vec::new(), withheld: Vec::new() });
                continue;
            }
            tokio::select! {
                _ = transport.stopped(), if !shutting_down => {
                    return Err(ServiceFailure::Transport);
                }
                // Export buffers can be freed after a pending-limit receipt is
                // sent. Observe actual reclamation even if no task/port event
                // follows. This timer exists only for pending resource work;
                // a healthy idle root awaits events without polling for tasks.
                _ = tokio::time::sleep_until(heap_inspection.unwrap_or_else(tokio::time::Instant::now)),
                    if heap_inspection.is_some() && !shutting_down => {}
                admission = admissions.recv(), if !shutting_down => {
                    let Some(admission) = admission else {
                        shutting_down = true;
                        closed.store(true, Ordering::Release);
                        shutdown.admission_credits.close();
                        for task in tasks.values() { task.control.cancel(); }
                        continue;
                    };
                    queued.push_back(admission);
                }
                Some(result) = calls.next(), if !calls.is_empty() => {
                    let record = tasks.get_mut(&result.task).ok_or(ServiceFailure::Protocol)?;
                    if result.panicked { return Err(ServiceFailure::InvalidPortResult); }
                    if let Some(finished) = result.finished {
                        record.outcome = Some(finished.map_err(|_| ServiceFailure::InvalidPortResult)?);
                        if record.control.0.cancelled.load(Ordering::Acquire) {
                            record.outcome = Some(TaskOutcome::Failed { reason_kind: "task_cancelled".into() });
                        }
                    }
                    let answer = match result.result {
                        Ok(value) => PortAnswer::Return { value },
                        Err(error) => PortAnswer::DomainError { reason_kind: error.reason },
                    };
                    snapshot = exchange(&transport, &mut exchanges, WorkerCommand::Resolve { key: result.key, answer }).await?;
                    for withheld in snapshot.withheld_answers.drain(..) {
                        if withheld.continuation != result.key { return Err(ServiceFailure::Protocol); }
                        record.withheld.push(withheld);
                    }
                }
                _ = changed.notified() => {}
                stopped = stop.changed(), if !shutting_down => {
                    if stopped.is_err() { stop = watch::channel(true).1; }
                }
            }
        }
    }.await;

    closed.store(true, Ordering::Release);
    shutdown.admission_credits.close();
    shutdown.settings.requests.close();
    while let Ok(publication) = shutdown.settings.requests.try_recv() {
        publication.reject(ServiceFailure::Closed);
    }
    admissions.close();
    while let Ok(admission) = admissions.try_recv() {
        queued.push_back(admission);
    }
    for admission in queued {
        queued_evidence.push(reject(
            admission,
            result
                .as_ref()
                .err()
                .copied()
                .unwrap_or(ServiceFailure::Closed),
        ));
    }
    if result.is_err() {
        for record in tasks.values() {
            record.control.cancel();
        }
        if let Some(admission) = &pending_admission {
            admission.control.cancel();
        }
        owner.request_termination();
    }
    // Observe real containment and settle every retained actual host future.
    // A turn waiter may time out, but this owning task never aborts the futures.
    let transport_exit = owner.join().await.map_err(|_| ServiceFailure::Transport);
    while let Some(result) = calls.next().await {
        if let Some(record) = tasks.get_mut(&result.task) {
            if let Ok(value) = result.result {
                record.retained.push(value);
            }
            if let Some(Ok(outcome)) = result.finished {
                record.outcome = Some(outcome);
            }
        }
    }
    let failure = result
        .err()
        .or_else(|| transport_exit.as_ref().err().copied());
    let pending_admission = pending_admission.map(|admission| {
        admission
            .control
            .0
            .result
            .send_replace(Some(Err(failure.unwrap_or(ServiceFailure::Protocol))));
        AdmissionEvidence {
            input: admission.input,
            ports: admission.control.0.ports.clone(),
        }
    });
    let mut evidence = Vec::new();
    for (task, record) in tasks {
        record
            .control
            .0
            .result
            .send_replace(Some(Err(failure.unwrap_or(ServiceFailure::Protocol))));
        evidence.push(TaskEvidence {
            task,
            ports: record.control.0.ports.clone(),
            host_results: record.retained,
            reported_outcome: record.outcome,
            withheld: record.withheld,
        });
    }
    ServiceExit {
        root: shutdown.root,
        failure,
        transport: transport_exit,
        tasks: evidence,
        pending_admission,
        queued_admissions: queued_evidence,
        failed_exchanges: exchanges.failed,
        rejected_commands: exchanges.rejected,
        transport_inbox: transport.completions(),
        last_snapshot: snapshot,
    }
}

fn reject(admission: Admission, reason: ServiceFailure) -> AdmissionEvidence {
    admission.control.0.ports.fence();
    admission.control.0.result.send_replace(Some(Err(reason)));
    AdmissionEvidence {
        input: admission.input,
        ports: admission.control.0.ports.clone(),
    }
}

/// CloseWorker can yield real interpreter control before reaching the next
/// parked future or final return. Advance those boundaries before closing the
/// next worker; an old work key is not resumable from a ControlYield state.
async fn settle_shutdown_boundary(
    transport: &TransportClient,
    evidence: &mut ExchangeEvidence,
    snapshot: &mut ProcessSnapshot,
    timeout: Duration,
) -> Result<(), ServiceFailure> {
    tokio::time::timeout(timeout, async {
        loop {
            match snapshot.boundary.take() {
                Some(ProcessBoundary::ControlYield { key }) => {
                    tokio::task::yield_now().await;
                    *snapshot =
                        exchange(transport, evidence, WorkerCommand::ResumeControl { key }).await?;
                }
                Some(ProcessBoundary::HostCall { key, name, .. })
                    if name == "await_next_task"
                        && snapshot
                            .work_waits
                            .iter()
                            .any(|(_, waiting)| *waiting == key) =>
                {
                    *snapshot = exchange(transport, evidence, WorkerCommand::Defer { key }).await?;
                }
                Some(ProcessBoundary::HostCall { .. }) => return Err(ServiceFailure::Protocol),
                _ => return Ok(()),
            }
        }
    })
    .await
    .map_err(|_| ServiceFailure::Deadline)?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinishInput {
    status: String,
    reply_ref: Option<String>,
    reason_kind: Option<String>,
}
fn parse_finish(value: Value) -> Result<TaskOutcome, ServiceFailure> {
    let result: FinishInput =
        serde_json::from_value(value).map_err(|_| ServiceFailure::InvalidPortResult)?;
    match (result.status.as_str(), result.reply_ref, result.reason_kind) {
        ("completed", Some(reply_ref), None)
            if reply_ref.starts_with("msg:") && reply_ref.len() > 4 =>
        {
            Ok(TaskOutcome::Completed { reply_ref })
        }
        ("failed", None, Some(reason_kind)) if valid_reason(&reason_kind) => {
            Ok(TaskOutcome::Failed { reason_kind })
        }
        _ => Err(ServiceFailure::InvalidPortResult),
    }
}
