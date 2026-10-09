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
    sync::{Notify, mpsc, watch},
    task::JoinHandle,
};

use crate::admission_capacity::{AdmissionCapacity, AdmissionCredit};
pub use crate::admission_capacity::{AdmissionLimits, AdmissionObservation};

use crate::{
    ContinuationKey,
    global::{Lifecycle, RootExecutionIdentity},
    heap::{HeapSettings, HeapStatus},
    process::{
        PortAnswer, ProcessBoundary, ProcessFailure, ProcessLimits, ProcessSnapshot, RecipeCommand,
        RecipeEvent, RootBoot, TaskHandle, TaskSettings, WorkerCommand,
    },
    transport_actor::{
        ActorExit, ActorFailure, ActorLimits, HostingDeadlines, RuntimeCommit, StartError,
        TransportClient, TransportOwner, TransportReceipt,
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
    Completed {
        reply_ref: String,
    },
    /// A host-owned internal task produces a durable receipt, never a chat
    /// publication. Its TaskPorts owner must verify the exact retained receipt.
    InternalCompleted {
        receipt_ref: String,
    },
    Failed {
        reason_kind: String,
    },
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

    /// Verify a completed reply against this task's published transcript, or
    /// an internal completion against its actual durable receipt. A chat owner
    /// must reject internal completion. Failures retain actual effect evidence.
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
    values: Option<crate::VmBounds>,
    max_recipe_contexts: Option<u32>,
    admission_limits: Option<AdmissionLimits>,
    max_pending_settings: Option<u32>,
    max_retained_attempts: Option<u32>,
    actor_limits: Option<ActorLimits>,
    hosting_deadlines: Option<HostingDeadlines>,
    adapter_reserve_bytes: Option<usize>,
    heap_update: Option<crate::heap::HeapUpdate>,
    durable: Option<DurableSettingsCommit>,
    result: watch::Sender<Option<Result<Arc<SettingsReceipt>, ServiceFailure>>>,
}
#[derive(Default)]
struct HostingPolicyPatch {
    admission: Option<AdmissionLimits>,
    pending_settings: Option<u32>,
    retained_attempts: Option<u32>,
    actor: Option<ActorLimits>,
    deadlines: Option<HostingDeadlines>,
    adapter_reserve_bytes: Option<usize>,
}
struct HeapPublication {
    expected: u64,
    settings: HeapSettings,
    automatic: bool,
    result: watch::Sender<Option<Result<HeapObservation, ServiceFailure>>>,
}
pub use crate::transport_actor::{DurableSettingsCommit, HeapCommitOutcome};
struct HeapTransaction {
    expected: u64,
    settings: HeapSettings,
    commit: BoxFuture<'static, HeapCommitOutcome>,
    commit_timeout: Duration,
    result: watch::Sender<Option<Result<HeapObservation, ServiceFailure>>>,
}
struct MemoryAdmissionPublication {
    expected: u64,
    backpressure: bool,
    result: watch::Sender<Option<Result<MemoryAdmissionPolicy, ServiceFailure>>>,
}
enum SettingsPublication {
    Task(Box<TaskSettingsPublication>),
    Heap(HeapPublication),
    HeapTransaction(HeapTransaction),
    MemoryAdmission(MemoryAdmissionPublication),
}
struct CreditedPublication {
    publication: SettingsPublication,
    // Retain ownership through execution, including a durable heap transaction.
    _credit: AdmissionCredit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsCapacityObservation {
    pub limit: u32,
    pub pending: u32,
}
impl SettingsCapacityObservation {
    pub fn over_capacity(self) -> bool {
        self.pending > self.limit
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceHostingLimits {
    /// None preserves the worker's current adapter headroom.
    pub adapter_reserve_bytes: Option<usize>,
    pub admission: AdmissionLimits,
    pub max_pending_settings: u32,
    pub max_retained_attempts: u32,
    /// None preserves the current actor policy for legacy hosting callers.
    pub actor: Option<ActorLimits>,
    pub deadlines: Option<HostingDeadlines>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedAttemptObservation {
    pub limit: u32,
    pub retained: u32,
}
impl RetainedAttemptObservation {
    pub fn over_capacity(self) -> bool {
        self.retained > self.limit
    }
}

/// Resource ownership only. The trusted attempt owner shares one credit across
/// preparation/driver/reconciliation; it supplies no claim or Tool permission.
pub struct RetainedAttemptCredit {
    _credit: AdmissionCredit,
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
            Self::HeapTransaction(publication) => {
                publication.result.send_replace(Some(Err(failure)));
            }
            Self::MemoryAdmission(publication) => {
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

/// Instance-service admission state. Its sequence is separate from persisted
/// operator settings and worker heap revisions. It never cancels active work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryAdmissionPolicy {
    pub revision: u64,
    pub backpressure: bool,
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
    pub heap: HeapObservation,
    pub allocator: crate::heap::AllocatorStatus,
    pub effective_settings: TaskSettings,
    pub recipe_context_capacity: crate::process::RecipeContextCapacity,
    pub admission: AdmissionObservation,
    pub settings_capacity: SettingsCapacityObservation,
    pub retained_attempts: RetainedAttemptObservation,
    pub actor_capacity: crate::transport_actor::ActorCapacityObservation,
    pub hosting_deadlines: HostingDeadlines,
    pub accounting: Vec<crate::process::TaskAccounting>,
}
struct SettingsInbox {
    requests: mpsc::UnboundedReceiver<CreditedPublication>,
    credits: Arc<AdmissionCapacity>,
    live: LiveMontyTaskSettings,
    heap: watch::Sender<HeapObservation>,
    memory_admission: watch::Sender<MemoryAdmissionPolicy>,
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
    _credit: AdmissionCredit,
}
#[derive(Clone)]
pub struct ServiceClient {
    transport: TransportClient,
    admissions: mpsc::UnboundedSender<Admission>,
    admission_credits: Arc<AdmissionCapacity>,
    retained_credits: Arc<AdmissionCapacity>,
    changed: Arc<Notify>,
    closed: Arc<AtomicBool>,
    closed_wake: Arc<Notify>,
    values: watch::Receiver<crate::VmBounds>,
    recipe_contexts: watch::Receiver<crate::process::RecipeContextCapacity>,
    root_source_bytes: usize,
    root: RootExecutionIdentity,
    worker_process_id: Option<u32>,
    settings: mpsc::UnboundedSender<CreditedPublication>,
    settings_credits: Arc<AdmissionCapacity>,
    effective_settings: LiveMontyTaskSettings,
    observed_heap: watch::Receiver<HeapObservation>,
    memory_admission: watch::Receiver<MemoryAdmissionPolicy>,
}
impl ServiceClient {
    /// Fence new admissions synchronously. The retained ServiceOwner still
    /// performs shutdown/cancellation and observes actual started host futures.
    pub fn close_admission(&self) {
        self.closed.store(true, Ordering::Release);
        self.closed_wake.notify_waiters();
        self.admission_credits.close();
        self.retained_credits.close();
        self.settings_credits.close();
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
        self.publish_runtime(
            expected,
            settings,
            None,
            None,
            HostingPolicyPatch::default(),
            None,
        )
        .await
    }

    pub fn root_identity(&self) -> RootExecutionIdentity {
        self.root
    }

    pub fn vm_bounds(&self) -> crate::VmBounds {
        *self.values.borrow()
    }

    pub fn recipe_context_capacity(&self) -> crate::process::RecipeContextCapacity {
        *self.recipe_contexts.borrow()
    }

    pub fn allocator_status(&self) -> crate::heap::AllocatorStatus {
        self.transport.allocator_status()
    }

    pub fn validate_adapter_reserve(&self, reserve: usize) -> Result<(), ServiceFailure> {
        let heap = self.observed_heap.borrow().status;
        for heap in [heap.desired, heap.effective].into_iter().flatten() {
            self.validate_memory_layout(heap.max_vm_bytes, reserve)?;
        }
        Ok(())
    }

    pub fn validate_memory_layout(
        &self,
        heap: usize,
        reserve: usize,
    ) -> Result<(), ServiceFailure> {
        if heap == 0
            || self
                .transport
                .frame_limit()
                .checked_mul(2)
                .and_then(|frames| frames.checked_add(reserve))
                .and_then(|non_vm| heap.checked_add(non_vm))
                .is_none_or(|bytes| bytes == usize::MAX)
        {
            return Err(ServiceFailure::InvalidLimits);
        }
        Ok(())
    }

    pub fn hosting_deadlines(&self) -> Result<HostingDeadlines, ServiceFailure> {
        self.transport
            .hosting_deadlines()
            .map_err(|_| ServiceFailure::Transport)
    }
    pub fn validate_vm_bounds(&self, values: crate::VmBounds) -> Result<(), ServiceFailure> {
        self.validate_runtime_bounds(values, self.hosting_deadlines()?.response_timeout)
    }
    pub fn validate_runtime_bounds(
        &self,
        values: crate::VmBounds,
        response_timeout: Duration,
    ) -> Result<(), ServiceFailure> {
        if !crate::process::runtime_bounds_supported(values, self.transport.frame_limit())
            || response_timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(response_timeout)
                .is_none()
            || values.execution_slice >= response_timeout
            || values.max_source_bytes < self.root_source_bytes
        {
            return Err(ServiceFailure::InvalidLimits);
        }
        self.transport
            .validate_execution_slice(values.execution_slice)
            .map_err(|_| ServiceFailure::Backpressure)?;
        Ok(())
    }

    pub async fn publish_runtime_settings(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: crate::VmBounds,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        self.validate_vm_bounds(values)?;
        self.publish_runtime(
            expected,
            settings,
            Some(values),
            None,
            HostingPolicyPatch::default(),
            None,
        )
        .await
    }

    /// Publish child capacity and VM bounds under the same worker-acknowledged
    /// settings generation. Reductions retain owned contexts and their state.
    pub async fn publish_execution_settings(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: crate::VmBounds,
        max_recipe_contexts: u32,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        self.validate_vm_bounds(values)?;
        if max_recipe_contexts == 0 {
            return Err(ServiceFailure::InvalidLimits);
        }
        self.publish_runtime(
            expected,
            settings,
            Some(values),
            Some(max_recipe_contexts),
            HostingPolicyPatch::default(),
            None,
        )
        .await
    }

    /// Queue policy is Rust-owned, but becomes effective in the same serialized
    /// slot as the worker acknowledgement. Existing work keeps its credits.
    pub async fn publish_hosting_settings(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: crate::VmBounds,
        max_recipe_contexts: u32,
        admission_limits: AdmissionLimits,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        self.validate_vm_bounds(values)?;
        if max_recipe_contexts == 0 || !admission_limits.valid() {
            return Err(ServiceFailure::InvalidLimits);
        }
        self.publish_runtime(
            expected,
            settings,
            Some(values),
            Some(max_recipe_contexts),
            HostingPolicyPatch {
                admission: Some(admission_limits),
                ..HostingPolicyPatch::default()
            },
            None,
        )
        .await
    }

    /// Publish all hosting controls in one acknowledged settings generation.
    pub async fn publish_control_settings(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: crate::VmBounds,
        max_recipe_contexts: u32,
        hosting: ServiceHostingLimits,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        self.publish_controls(
            expected,
            settings,
            values,
            max_recipe_contexts,
            hosting,
            None,
        )
        .await
    }

    /// Actual allocator feasibility precedes the owned durable operation. The
    /// sole transport owner excludes child RPCs until the exact worker/Rust ACK.
    /// Dropping an HTTP waiter cannot cancel the accepted commit or publication.
    pub async fn publish_control_settings_transaction(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: crate::VmBounds,
        max_recipe_contexts: u32,
        hosting: ServiceHostingLimits,
        durable: DurableSettingsCommit,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        if durable.timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(durable.timeout)
                .is_none()
        {
            return Err(ServiceFailure::InvalidLimits);
        }
        self.publish_controls(
            expected,
            settings,
            values,
            max_recipe_contexts,
            hosting,
            Some(durable),
        )
        .await
    }

    async fn publish_controls(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: crate::VmBounds,
        max_recipe_contexts: u32,
        hosting: ServiceHostingLimits,
        durable: Option<DurableSettingsCommit>,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        if let Some(update) = durable.as_ref().and_then(|durable| durable.heap_update) {
            self.validate_memory_layout(
                update.settings.max_vm_bytes,
                hosting
                    .adapter_reserve_bytes
                    .unwrap_or(self.allocator_status().adapter_reserve_bytes),
            )?;
        } else if let Some(reserve) = hosting.adapter_reserve_bytes {
            self.validate_adapter_reserve(reserve)?;
        }
        if hosting
            .deadlines
            .is_some_and(|deadlines| !deadlines.valid())
        {
            return Err(ServiceFailure::InvalidLimits);
        }
        self.validate_runtime_bounds(
            values,
            hosting
                .deadlines
                .unwrap_or(self.hosting_deadlines()?)
                .response_timeout,
        )?;
        if let Some(actor) = hosting.actor {
            self.validate_actor_limits(actor)?;
        }
        if max_recipe_contexts == 0
            || !hosting.admission.valid()
            || hosting.max_pending_settings == 0
            || hosting.max_retained_attempts == 0
        {
            return Err(ServiceFailure::InvalidLimits);
        }
        self.publish_runtime(
            expected,
            settings,
            Some(values),
            Some(max_recipe_contexts),
            HostingPolicyPatch {
                admission: Some(hosting.admission),
                pending_settings: Some(hosting.max_pending_settings),
                retained_attempts: Some(hosting.max_retained_attempts),
                actor: hosting.actor,
                deadlines: hosting.deadlines,
                adapter_reserve_bytes: hosting.adapter_reserve_bytes,
            },
            durable,
        )
        .await
    }

    pub fn validate_actor_limits(&self, limits: ActorLimits) -> Result<(), ServiceFailure> {
        self.transport
            .validate_limits(limits)
            .map_err(|_| ServiceFailure::InvalidLimits)
    }
    pub fn actor_capacity(
        &self,
    ) -> Result<crate::transport_actor::ActorCapacityObservation, ServiceFailure> {
        self.transport
            .capacity()
            .map_err(|_| ServiceFailure::Transport)
    }

    pub fn settings_capacity(&self) -> SettingsCapacityObservation {
        let observed = self.settings_credits.observation();
        SettingsCapacityObservation {
            limit: observed.limits.max_tasks,
            pending: observed.tasks,
        }
    }

    pub fn retained_attempts(&self) -> RetainedAttemptObservation {
        let observed = self.retained_credits.observation();
        RetainedAttemptObservation {
            limit: observed.limits.max_tasks,
            retained: observed.tasks,
        }
    }

    /// Reserve before asynchronous task preparation. Sharing/deduplicating this
    /// credit belongs to the private Rust attempt owner, never a Python token.
    pub fn try_retain_attempt(&self) -> Result<RetainedAttemptCredit, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        Ok(RetainedAttemptCredit {
            _credit: self.retained_credits.try_reserve(0)?,
        })
    }

    fn enqueue_settings(&self, publication: SettingsPublication) -> Result<(), ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        // The control lane has no additional byte-budget policy. Its payloads
        // remain subject to the worker transport's validated frame bounds.
        let credit = self.settings_credits.try_reserve(0)?;
        self.settings
            .send(CreditedPublication {
                publication,
                _credit: credit,
            })
            .map_err(|_| ServiceFailure::Closed)?;
        self.changed.notify_one();
        Ok(())
    }

    pub fn admission_observation(&self) -> AdmissionObservation {
        self.admission_credits.observation()
    }

    async fn publish_runtime(
        &self,
        expected: u64,
        settings: TaskSettings,
        values: Option<crate::VmBounds>,
        max_recipe_contexts: Option<u32>,
        hosting: HostingPolicyPatch,
        durable: Option<DurableSettingsCommit>,
    ) -> Result<Arc<SettingsReceipt>, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        let (result, mut receipt) = watch::channel(None);
        self.enqueue_settings(SettingsPublication::Task(Box::new(
            TaskSettingsPublication {
                expected,
                settings,
                values,
                max_recipe_contexts,
                admission_limits: hosting.admission,
                max_pending_settings: hosting.pending_settings,
                max_retained_attempts: hosting.retained_attempts,
                actor_limits: hosting.actor,
                hosting_deadlines: hosting.deadlines,
                adapter_reserve_bytes: hosting.adapter_reserve_bytes,
                heap_update: durable.as_ref().and_then(|durable| durable.heap_update),
                durable,
                result,
            },
        )))?;
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

    pub fn worker_process_id(&self) -> Option<u32> {
        self.worker_process_id
    }

    /// Check and temporarily apply a manual limit at the serialized worker
    /// boundary, then hold every VM advancement until durable commit evidence.
    /// Host futures remain owned. A rejected commit restores the previous limit
    /// before any execution resumes; an unknown commit contains the service.
    /// The supplied future is polled only after actual worker feasibility.
    pub async fn publish_heap_transaction(
        &self,
        expected: u64,
        settings: HeapSettings,
        commit: BoxFuture<'static, HeapCommitOutcome>,
    ) -> Result<HeapObservation, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        let (result, mut receipt) = watch::channel(None);
        self.enqueue_settings(SettingsPublication::HeapTransaction(HeapTransaction {
            expected,
            settings,
            commit,
            commit_timeout: self.hosting_deadlines()?.response_timeout,
            result,
        }))?;
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

    pub fn heap_observation(&self) -> HeapObservation {
        *self.observed_heap.borrow()
    }

    pub fn memory_admission_policy(&self) -> MemoryAdmissionPolicy {
        *self.memory_admission.borrow()
    }

    /// Serialize pressure/measurement availability with actual admission on the
    /// instance control lane. Stale measurements cannot clear newer pressure.
    /// Dropping the waiter does not withdraw an accepted publication.
    pub async fn publish_memory_backpressure(
        &self,
        expected: u64,
        backpressure: bool,
    ) -> Result<MemoryAdmissionPolicy, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        let (result, mut receipt) = watch::channel(None);
        self.enqueue_settings(SettingsPublication::MemoryAdmission(
            MemoryAdmissionPublication {
                expected,
                backpressure,
                result,
            },
        ))?;
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

    /// Wait for bounded queue capacity and safe memory admission without a work
    /// polling loop or repeated input/history cloning. Caller-held inputs
    /// awaiting capacity are not admitted work. Cancellation before enqueue
    /// owns no VM task and returns any acquired queue credit.
    pub async fn submit_when_available(
        &self,
        input: TaskInput,
        ports: Arc<dyn TaskPorts>,
    ) -> Result<TaskTicket, ServiceFailure> {
        self.admission_credits.wait_available().await?;
        let (input, bytes) = self.prepare_input(input)?;
        let credit = self.admission_credits.reserve(bytes).await?;
        let mut memory = self.memory_admission.clone();
        let mut heap = self.observed_heap.clone();
        loop {
            let closed = self.closed_wake.notified();
            tokio::pin!(closed);
            closed.as_mut().enable();
            if self.closed.load(Ordering::Acquire) {
                return Err(ServiceFailure::Closed);
            }
            let blocked = memory.borrow_and_update().backpressure;
            let pending = heap.borrow_and_update().status.pending_reduction;
            if !blocked && !pending {
                return self.submit_with_credit(input, ports, credit);
            }
            tokio::select! {
                result = memory.changed() => { result.map_err(|_| ServiceFailure::Closed)?; }
                result = heap.changed() => { result.map_err(|_| ServiceFailure::Closed)?; }
                _ = closed => {}
            }
        }
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
        self.enqueue_settings(SettingsPublication::Heap(HeapPublication {
            expected,
            settings,
            automatic,
            result,
        }))?;
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
        if self.heap_observation().status.pending_reduction
            || self.memory_admission_policy().backpressure
        {
            return Err(ServiceFailure::Backpressure);
        }
        self.admission_credits.available()?;
        let (input, bytes) = self.prepare_input(input)?;
        let credit = self.admission_credits.try_reserve(bytes)?;
        self.submit_with_credit(input, ports, credit)
    }

    fn prepare_input(&self, input: TaskInput) -> Result<(Value, usize), ServiceFailure> {
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
        let observed_values = self.vm_bounds();
        let observed_frame = self.transport.frame_limit();
        if input.history.len() > observed_values.max_value_nodes
            || [
                &input.conversation_id,
                &input.message_id,
                &input.turn_id,
                &input.run_id,
                &input.user_input,
            ]
            .iter()
            .any(|value| value.len() > observed_values.max_value_bytes)
            || input.history.iter().any(|value| {
                !crate::process::transport_value(value, observed_values, observed_frame)
            })
        {
            return Err(ServiceFailure::InvalidInput);
        }
        let input = serde_json::to_value(input).map_err(|_| ServiceFailure::InvalidInput)?;
        // Reserve the worker-added task token before admission. Serialization
        // uses the largest possible continuation ordinal and a fixed-size UUID.
        let mut values = observed_values;
        values.max_value_nodes = values.max_value_nodes.saturating_sub(2);
        values.max_value_bytes = values.max_value_bytes.saturating_sub(64);
        let probe = WorkerCommand::Admit {
            key: ContinuationKey {
                vm_id: uuid::Uuid::nil(),
                ordinal: u64::MAX,
            },
            task: input,
        };
        let bytes = crate::process::retain_command(&probe, values, observed_frame)
            .map_err(|_| ServiceFailure::InvalidInput)?
            .len();
        let WorkerCommand::Admit { task, .. } = probe else {
            return Err(ServiceFailure::InvalidInput);
        };
        Ok((task, bytes))
    }

    fn submit_with_credit(
        &self,
        input: Value,
        ports: Arc<dyn TaskPorts>,
        credit: AdmissionCredit,
    ) -> Result<TaskTicket, ServiceFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServiceFailure::Closed);
        }
        let (result, _) = watch::channel(None);
        let control = TaskControl(Arc::new(Control {
            cancelled: AtomicBool::new(false),
            changed: Arc::clone(&self.changed),
            ports,
            result,
        }));
        self.admissions
            .send(Admission {
                input,
                control: control.clone(),
                _credit: credit,
            })
            .map_err(|_| ServiceFailure::Closed)?;
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
        let max_tasks = u32::try_from(queue_capacity)
            .map_err(|_| StartError::Actor(crate::transport_actor::ActorFailure::InvalidLimits))?;
        Self::start_with_admission_limits(
            executable,
            boot,
            process,
            actor,
            AdmissionLimits {
                max_tasks,
                max_bytes: 64 * 1024 * 1024,
            },
            live,
        )
        .await
    }

    pub async fn start_with_admission_limits(
        executable: &Path,
        boot: RootBoot,
        process: ProcessLimits,
        actor: ActorLimits,
        admission_limits: AdmissionLimits,
        live: LiveMontyTaskSettings,
    ) -> Result<Self, StartError> {
        Self::start_with_hosting_limits(
            executable,
            boot,
            process,
            actor,
            ServiceHostingLimits {
                adapter_reserve_bytes: None,
                admission: admission_limits,
                max_pending_settings: 8,
                max_retained_attempts: 256,
                actor: None,
                deadlines: None,
            },
            live,
        )
        .await
    }

    pub async fn start_with_hosting_limits(
        executable: &Path,
        boot: RootBoot,
        process: ProcessLimits,
        actor: ActorLimits,
        hosting: ServiceHostingLimits,
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
        if !hosting.admission.valid()
            || hosting.max_pending_settings == 0
            || hosting.max_retained_attempts == 0
        {
            return Err(StartError::Actor(
                crate::transport_actor::ActorFailure::InvalidLimits,
            ));
        }
        if !crate::process::runtime_bounds_supported(boot.bounds.values, process.max_frame_bytes)
            || boot.bounds.values.execution_slice >= process.response_timeout
        {
            return Err(StartError::Actor(
                crate::transport_actor::ActorFailure::InvalidLimits,
            ));
        }
        if hosting.deadlines.is_some_and(|configured| {
            !configured.valid()
                || configured.startup_timeout != boot.startup_timeout
                || configured.response_timeout != process.response_timeout
        }) {
            return Err(StartError::Actor(
                crate::transport_actor::ActorFailure::InvalidLimits,
            ));
        }
        if hosting.actor.is_some_and(|configured| configured != actor) {
            return Err(StartError::Actor(
                crate::transport_actor::ActorFailure::InvalidLimits,
            ));
        }
        let root_source_bytes = boot.source.len();
        let workers = boot.bounds.workers;
        let (owner, ready) = TransportOwner::start(executable, boot, process, actor).await?;
        let worker_process_id = owner.worker_process_id();
        let root = ready.root.expect("transport verified root identity");
        // start_ready proves every live worker is parked before returning.
        // Only validated, count-and-byte-reserved work may enter this channel.
        // Physical channel size does not impose an additional live-update cap.
        let (admissions, rx) = mpsc::unbounded_channel();
        let admission_credits = AdmissionCapacity::new(hosting.admission)
            .map_err(|_| StartError::Actor(crate::transport_actor::ActorFailure::InvalidLimits))?;
        // Logical credits cover both queued and executing publications. A live
        // increase must not be constrained by an old physical channel size.
        let (settings, settings_rx) = mpsc::unbounded_channel();
        let settings_credits = AdmissionCapacity::new(AdmissionLimits {
            max_tasks: hosting.max_pending_settings,
            max_bytes: 1,
        })
        .map_err(|_| StartError::Actor(crate::transport_actor::ActorFailure::InvalidLimits))?;
        let (heap, observed_heap) = watch::channel(HeapObservation::from(&ready));
        let retained_credits = AdmissionCapacity::new(AdmissionLimits {
            max_tasks: hosting.max_retained_attempts,
            max_bytes: 1,
        })
        .map_err(|_| StartError::Actor(crate::transport_actor::ActorFailure::InvalidLimits))?;
        let (memory_admission, observed_memory_admission) = watch::channel(MemoryAdmissionPolicy {
            revision: 0,
            backpressure: false,
        });
        let (shutdown, stop) = watch::channel(false);
        let changed = Arc::new(Notify::new());
        let closed = Arc::new(AtomicBool::new(false));
        let closed_wake = Arc::new(Notify::new());
        let client = ServiceClient {
            transport: owner.client(),
            admissions,
            admission_credits: admission_credits.clone(),
            retained_credits: retained_credits.clone(),
            changed: changed.clone(),
            closed: closed.clone(),
            closed_wake: closed_wake.clone(),
            values: owner.client().live_vm_bounds(),
            recipe_contexts: owner.client().live_recipe_contexts(),
            root_source_bytes,
            root,
            worker_process_id,
            settings,
            settings_credits: settings_credits.clone(),
            effective_settings: live.clone(),
            observed_heap,
            memory_admission: observed_memory_admission,
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
                retained_credits,
                closed_wake,
                workers,
                settings: SettingsInbox {
                    requests: settings_rx,
                    credits: settings_credits,
                    live,
                    heap,
                    memory_admission,
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
    admission_credits: Arc<AdmissionCapacity>,
    retained_credits: Arc<AdmissionCapacity>,
    closed_wake: Arc<Notify>,
    workers: u32,
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

/// Expected pre-publication resource denials leave the task policy and worker
/// alive. Unexpected/uncertain outcomes retain their receipt and are contained.
async fn update_runtime(
    client: &TransportClient,
    evidence: &mut ExchangeEvidence,
    command: WorkerCommand,
    previous: &ProcessSnapshot,
    commit: RuntimeCommit,
    durable: Option<DurableSettingsCommit>,
) -> Result<(ProcessSnapshot, Option<ServiceFailure>), ServiceFailure> {
    let transactional = durable.is_some();
    let ticket = match durable {
        Some(durable) => client.try_submit_runtime_transaction(command, commit, durable),
        None => client.try_submit_runtime_publication(command, commit),
    }
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
            ProcessFailure::Vm(crate::VmFailure::InvalidBounds) => {
                Some(ServiceFailure::InvalidLimits)
            }
            ProcessFailure::Vm(crate::VmFailure::SettingsRevisionConflict) => {
                Some(ServiceFailure::SettingsConflict)
            }
            _ => None,
        },
        Ok(_) => None,
    };
    if receipt.outcome.is_err() && denial.is_none() {
        evidence.failed.push(receipt);
        return Err(ServiceFailure::Transport);
    }
    if denial.is_some() && receipt.outcome.as_ref().err().and_then(|error| error.snapshot.as_deref())
        .is_none_or(|next| next.boundary.is_some() || next.admitted_task.is_some()
            || !next.withheld_answers.is_empty()
            || !(matches!(&next.recipe, Some(RecipeEvent::Failed { context: None, stdout }) if stdout.is_empty())
                || (transactional && next.recipe.is_none() && next.stdout.is_empty()))
            // Child opens/closes can precede this control command. Their live
            // counts are not a policy revision and must not fabricate a denial
            // mismatch; the configured capacity must remain unchanged.
            || next.runtime_settings() != previous.runtime_settings()) {
        evidence.failed.push(receipt);
        return Err(ServiceFailure::Protocol);
    }
    let snapshot = match receipt.outcome {
        Ok(snapshot) => snapshot,
        Err(mut error) => *error.snapshot.take().ok_or(ServiceFailure::Protocol)?,
    };
    Ok((snapshot, denial))
}

/// Bounded synchronous metadata publication in the sole transport owner. The
/// service still owns sequencing, the unconsumed root boundary and the final
/// settings receipt; children cannot resume in the worker/Rust revision gap.
fn runtime_commit(
    shutdown: &ShutdownBounds,
    transport: &TransportClient,
    publication: &TaskSettingsPublication,
) -> RuntimeCommit {
    let expected = publication.expected;
    let settings = publication.settings;
    let values = publication.values;
    let contexts = publication.max_recipe_contexts;
    let reserve = publication.adapter_reserve_bytes;
    let heap_update = publication.heap_update;
    let admission_limits = publication.admission_limits;
    let pending = publication.max_pending_settings;
    let retained = publication.max_retained_attempts;
    let actor = publication.actor_limits;
    let deadlines = publication.hosting_deadlines;
    let live = shutdown.settings.live.clone();
    let heap = shutdown.settings.heap.clone();
    let admissions = shutdown.admission_credits.clone();
    let pending_credits = shutdown.settings.credits.clone();
    let retained_credits = shutdown.retained_credits.clone();
    let transport = transport.clone();
    Box::new(move |receipt| {
        if !matches!(&receipt.recipe, Some(RecipeEvent::SettingsUpdated))
            || receipt.effective_task_settings != Some(settings)
            || values.is_some_and(|values| receipt.vm_bounds != Some(values))
            || receipt.vm_bounds.is_none()
            || receipt.recipe_context_capacity.is_none()
            || contexts.is_some_and(|limit| {
                receipt
                    .recipe_context_capacity
                    .is_none_or(|capacity| capacity.limit != limit)
            })
            || reserve.is_some_and(|reserve| receipt.allocator.adapter_reserve_bytes != reserve)
            || heap_update.is_some_and(|update| {
                receipt.heap.desired != Some(update.settings)
                    || receipt.heap.effective != Some(update.settings)
                    || receipt.heap.pending_reduction
            })
            || receipt.boundary.is_some()
            || receipt.admitted_task.is_some()
            || !receipt.withheld_answers.is_empty()
            || !receipt.stdout.is_empty()
            || live.current().revision != expected
        {
            return Err(ActorFailure::Transport(ProcessFailure::Protocol));
        }
        if let Some(limits) = admission_limits {
            admissions
                .publish(limits)
                .map_err(|_| ActorFailure::AccountingUnavailable)?;
        }
        if let Some(limit) = pending {
            pending_credits
                .publish(AdmissionLimits {
                    max_tasks: limit,
                    max_bytes: 1,
                })
                .map_err(|_| ActorFailure::AccountingUnavailable)?;
        }
        if let Some(limit) = retained {
            retained_credits
                .publish(AdmissionLimits {
                    max_tasks: limit,
                    max_bytes: 1,
                })
                .map_err(|_| ActorFailure::AccountingUnavailable)?;
        }
        transport.publish_hosting_policy_for_bounds(
            actor,
            deadlines,
            receipt.vm_bounds.ok_or(ActorFailure::InvalidLimits)?,
        )?;
        heap.send_replace(HeapObservation::from(receipt));
        live.publish(expected, settings.into())
            .map_err(|_| ActorFailure::AccountingUnavailable)
    })
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
    commit: Option<(BoxFuture<'static, HeapCommitOutcome>, Duration)>,
) -> Result<Option<ServiceFailure>, ServiceFailure> {
    let submission = if let Some((commit, timeout)) = commit {
        client.try_submit_heap_transaction(expected, settings, commit, timeout)
    } else {
        client.try_submit(WorkerCommand::UpdateHeap {
            expected_revision: expected,
            settings,
            automatic,
        })
    };
    let ticket = submission.map_err(|error| {
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
    mut admissions: mpsc::UnboundedReceiver<Admission>,
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
    let mut settings_turn = true;
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
                shutdown.closed_wake.notify_waiters();
                shutdown.admission_credits.close();
                shutdown.retained_credits.close();
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
            if settings_turn && let Ok(CreditedPublication { publication, _credit }) = shutdown.settings.requests.try_recv() {
                settings_turn = false;
                if shutting_down {
                    publication.reject(ServiceFailure::Closed);
                    continue;
                }
                match publication {
                    SettingsPublication::Task(mut publication) => {
                        if publication.expected != shutdown.settings.live.current().revision
                            || publication.settings.revision <= publication.expected {
                            publication.result.send_replace(Some(Err(ServiceFailure::SettingsConflict)));
                        } else if LiveMontyTaskSettings::new(publication.settings.into()).is_err() {
                            publication.result.send_replace(Some(Err(ServiceFailure::InvalidLimits)));
                        } else if publication.values.is_some_and(|values|
                            transport.validate_execution_slice(values.execution_slice).is_err()) {
                            // Older accepted commands retain their own deadlines.
                            // Do not mutate the worker or contain a healthy service.
                            publication.result.send_replace(Some(Err(ServiceFailure::Backpressure)));
                        } else {
                            let durable = publication.durable.take();
                            let update = update_runtime(&transport, &mut exchanges, WorkerCommand::Recipe {
                                command: match publication.values {
                                    Some(values) => RecipeCommand::UpdateRuntimeSettings {
                                        expected_revision: publication.expected, settings: publication.settings, values,
                                        max_recipe_contexts: publication.max_recipe_contexts,
                                        adapter_reserve_bytes: publication.adapter_reserve_bytes,
                                        heap_update: publication.heap_update,
                                    },
                                    None => RecipeCommand::UpdateSettings {
                                        expected_revision: publication.expected, settings: publication.settings,
                                    },
                                },
                            }, &snapshot, runtime_commit(&shutdown, &transport, &publication), durable).await;
                            let (mut receipt, denial) = match update {
                                Ok(receipt) => receipt,
                                Err(error) => {
                                    publication.result.send_replace(Some(Err(error)));
                                    return Err(error);
                                }
                            };
                            if let Some(denial) = denial {
                                receipt.recipe = None;
                                receipt.boundary = snapshot.boundary.take();
                                snapshot = receipt;
                                publication.result.send_replace(Some(Err(denial)));
                                continue;
                            }
                            // UpdateSettings does not advance the root. Preserve its
                            // actual unconsumed call/control boundary; replacing it
                            // with the side-command's None would strand execution.
                            receipt.boundary = snapshot.boundary.take();
                            snapshot = receipt;
                            // Rust policy and worker observations were published
                            // before the actor released this successful ACK or
                            // allowed any subsequent child RPC to execute.
                            publication.result.send_replace(Some(Ok(Arc::new(SettingsReceipt {
                                heap: HeapObservation::from(&snapshot),
                                allocator: snapshot.allocator,
                                effective_settings: publication.settings,
                                recipe_context_capacity: snapshot.recipe_context_capacity.ok_or(ServiceFailure::Protocol)?,
                                admission: shutdown.admission_credits.observation(),
                                settings_capacity: {
                                    let observed = shutdown.settings.credits.observation();
                                    SettingsCapacityObservation { limit: observed.limits.max_tasks, pending: observed.tasks }
                                },
                                retained_attempts: {
                                    let observed = shutdown.retained_credits.observation();
                                    RetainedAttemptObservation { limit: observed.limits.max_tasks, retained: observed.tasks }
                                },
                                actor_capacity: transport.capacity().map_err(|_| ServiceFailure::Transport)?,
                                hosting_deadlines: transport.hosting_deadlines().map_err(|_| ServiceFailure::Transport)?,
                                accounting: snapshot.task_accounting.clone(),
                            }))));
                        }
                    }
                    SettingsPublication::Heap(publication) => {
                        let update = update_heap(&transport, &mut exchanges, &mut snapshot,
                            publication.expected, publication.settings, publication.automatic, None).await;
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
                    SettingsPublication::HeapTransaction(publication) => {
                        let update = update_heap(&transport, &mut exchanges, &mut snapshot,
                            publication.expected, publication.settings, false,
                            Some((publication.commit, publication.commit_timeout))).await;
                        shutdown.settings.heap.send_replace(HeapObservation::from(&snapshot));
                        match update {
                            Ok(denial) => {
                                publication.result.send_replace(Some(match denial {
                                    Some(error) => Err(error),
                                    None => Ok(HeapObservation::from(&snapshot)),
                                }));
                            }
                            Err(error) => {
                                publication.result.send_replace(Some(Err(error)));
                                return Err(error);
                            }
                        }
                    }
                    SettingsPublication::MemoryAdmission(publication) => {
                        let current = *shutdown.settings.memory_admission.borrow();
                        let result = if publication.expected != current.revision {
                            Err(ServiceFailure::SettingsConflict)
                        } else if let Some(revision) = current.revision.checked_add(1) {
                            let next = MemoryAdmissionPolicy { revision, backpressure: publication.backpressure };
                            shutdown.settings.memory_admission.send_replace(next);
                            Ok(next)
                        } else {
                            Err(ServiceFailure::SettingsConflict)
                        };
                        publication.result.send_replace(Some(result));
                    }
                }
                continue;
            }
            // A continuously replenished settings inbox cannot monopolize
            // cancellation, root slices or completion handling. Alternate a
            // control operation with an ordinary service turn.
            settings_turn = true;
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
                        transport.hosting_deadlines().map_err(|_| ServiceFailure::Transport)?.response_timeout).await?;
                    let key = snapshot.work_waits.first().ok_or(ServiceFailure::Protocol)?.1;
                    snapshot = exchange(&transport, &mut exchanges, WorkerCommand::CloseWorker { key }).await?;
                }
                settle_shutdown_boundary(&transport, &mut exchanges, &mut snapshot,
                    transport.hosting_deadlines().map_err(|_| ServiceFailure::Transport)?.response_timeout).await?;
                if snapshot.lifecycle != Lifecycle::Stopped { return Err(ServiceFailure::Protocol); }
                return Ok(());
            }

            let available = snapshot.work_waits.iter().find(|(worker, _)|
                !tasks.values().any(|task| task.worker == *worker)).copied();
            let memory_backpressure = shutdown.settings.memory_admission.borrow().backpressure;
            if !shutting_down && !snapshot.heap.pending_reduction
                && !memory_backpressure
                && let Some((worker, key)) = available
                && let Some(admission) = queued.pop_front() {
                // Recheck the complete typed input at the actual dispatch boundary:
                // queued input retains its original history; it is never truncated.
                let probe = WorkerCommand::Admit { key, task: admission.input.clone() };
                let mut values = transport.live_vm_bounds().borrow().to_owned();
                values.max_value_nodes = values.max_value_nodes.saturating_sub(2);
                values.max_value_bytes = values.max_value_bytes.saturating_sub(64);
                if crate::process::retain_command(&probe, values, transport.frame_limit()).is_err() {
                    drop(reject(admission, ServiceFailure::InvalidInput));
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
                // Notify permits coalesce. Remaining owned publications are
                // already ready work; yield once and revisit without polling
                // an idle instance or awaiting a new producer notification.
                _ = tokio::task::yield_now(), if !shutdown.settings.requests.is_empty() => {}
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
                        shutdown.closed_wake.notify_waiters();
                        shutdown.admission_credits.close();
                        shutdown.retained_credits.close();
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
                        Ok(value) => {
                            let probe = WorkerCommand::Resolve { key: result.key,
                                answer: PortAnswer::Return { value } };
                            let accepted = crate::process::retain_command(&probe, *transport.live_vm_bounds().borrow(), transport.frame_limit()).is_ok();
                            let WorkerCommand::Resolve { answer: PortAnswer::Return { value }, .. } = probe else { unreachable!() };
                            if !accepted {
                                // The effect has already happened. Preserve its exact
                                // result for reconciliation; fail this task instead
                                // of truncating it or killing/replaying the instance.
                                record.withheld.push(crate::global::WithheldHostAnswer {
                                    continuation: result.key, answer: crate::HostAnswer::Return(value),
                                });
                                PortAnswer::DomainError { reason_kind: "task_value_limit".into() }
                            } else { PortAnswer::Return { value } }
                        },
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
    shutdown.closed_wake.notify_waiters();
    shutdown.admission_credits.close();
    shutdown.retained_credits.close();
    shutdown.settings.credits.close();
    shutdown.settings.requests.close();
    while let Ok(publication) = shutdown.settings.requests.try_recv() {
        publication.publication.reject(ServiceFailure::Closed);
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
    receipt_ref: Option<String>,
}
fn parse_finish(value: Value) -> Result<TaskOutcome, ServiceFailure> {
    let result: FinishInput =
        serde_json::from_value(value).map_err(|_| ServiceFailure::InvalidPortResult)?;
    match (
        result.status.as_str(),
        result.reply_ref,
        result.reason_kind,
        result.receipt_ref,
    ) {
        ("completed", Some(reply_ref), None, None)
            if reply_ref.starts_with("msg:") && reply_ref.len() > 4 =>
        {
            Ok(TaskOutcome::Completed { reply_ref })
        }
        ("internal_completed", None, None, Some(receipt_ref))
            if receipt_ref.strip_prefix("review:").is_some_and(|id| {
                uuid::Uuid::parse_str(id).is_ok_and(|uuid| !uuid.is_nil() && uuid.to_string() == id)
            }) =>
        {
            Ok(TaskOutcome::InternalCompleted { receipt_ref })
        }
        ("failed", None, Some(reason_kind), None) if valid_reason(&reason_kind) => {
            Ok(TaskOutcome::Failed { reason_kind })
        }
        _ => Err(ServiceFailure::InvalidPortResult),
    }
}
