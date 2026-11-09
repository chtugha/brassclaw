//! One original Pure invocation inside an actual retained HostCall window.
//! No workflow selection, retries, approval or independent task/VM admission.

use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use brassclaw_monty_host::{
    ContinuationKey, VmFailure,
    global::{Lifecycle, RootExecutionIdentity},
    process::{
        ProcessFailure, PythonExceptionObservation, RecipeBoundary, RecipeCommand, RecipeContextId,
        RecipeEvent, TaskHandle, WorkerCommand,
    },
    transport_actor::{RequestTicket, TransportClient, TransportReceipt},
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef, value_contract::ContractLimits,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use super::{retained_recipe::RetainedExecutionError, retained_source::SealedPureInvocation};

/// Clones share both the one-shot fence and original transport custody. There is
/// no public constructor, actor accessor or arbitrary command/source operation.
#[derive(Clone)]
pub struct BoundPureContext {
    inner: Arc<Inner>,
}
struct Inner {
    transport: TransportClient,
    task: TaskHandle,
    parent: RecipeContextId,
    host_key: ContinuationKey,
    root: RootExecutionIdentity,
    limits: ContractLimits,
    active: AtomicBool,
    root_reconciliation_required: AtomicBool,
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    started: bool,
    context: Option<RecipeContextId>,
    pending: Option<RequestTicket>,
    received: Option<TransportReceipt>,
    expected_command_checksum: Option<[u8; 32]>,
    receipt_count: u64,
    receipt_chain: Option<[u8; 32]>,
    retained: BTreeMap<&'static str, Arc<NativePureReceipt>>,
    terminal_stage: Option<&'static str>,
    feeding_preload: bool,
    stage: Option<&'static str>,
    native_failure: Option<ProcessFailure>,
    failure_facts: Option<Value>,
    #[cfg(any(test, feature = "test-support"))]
    test_subject: Option<(uuid::Uuid, String)>,
}

/// Dropping the actual executor dispatch window fences even retained clones.
pub(super) struct InvocationWindow(Arc<Inner>);
impl Drop for InvocationWindow {
    fn drop(&mut self) {
        self.0.active.store(false, Ordering::Release);
    }
}

/// Exact native receipt bytes constructed solely from the real actor receipt.
/// The consumer may persist/reconcile this data, never submit worker commands.
pub struct NativePureReceipt {
    ordinal: u64,
    request_id: uuid::Uuid,
    previous: Option<[u8; 32]>,
    bytes: String,
    checksum: [u8; 32],
}
impl NativePureReceipt {
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub fn request_id(&self) -> uuid::Uuid {
        self.request_id
    }
    pub fn previous_checksum(&self) -> Option<[u8; 32]> {
        self.previous
    }
    pub fn bytes(&self) -> &str {
        &self.bytes
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
}
/// Observed candidate data, never a behavioral verdict or approval. Exceptions
/// retain their actual class; no caller expectation can classify them as passed.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PureCandidateOutcome {
    Return {
        value: Value,
    },
    PythonException {
        exception: PythonExceptionObservation,
    },
    ResultContractViolation {
        value: Value,
    },
}

/// Constructed only after the original invocation and acknowledged child release.
/// The case owner still must persist it provisionally and receive the ordinary
/// private harness delivery before settling a case.
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub struct PureInvocationObservation {
    format: &'static str,
    component: ComponentRevisionRef,
    export: String,
    task: TaskHandle,
    parent: RecipeContextId,
    host_key: ContinuationKey,
    context: RecipeContextId,
    inputs_checksum: [u8; 32],
    invocation_checksum: [u8; 32],
    outcome: PureCandidateOutcome,
}
impl PureInvocationObservation {
    pub fn outcome(&self) -> &PureCandidateOutcome {
        &self.outcome
    }
}

impl BoundPureContext {
    pub(super) fn at_host_call(
        transport: TransportClient,
        task: TaskHandle,
        parent: RecipeContextId,
        host_key: ContinuationKey,
        root: RootExecutionIdentity,
        limits: ContractLimits,
    ) -> (Self, InvocationWindow) {
        let inner = Arc::new(Inner {
            transport,
            task,
            parent,
            host_key,
            root,
            limits,
            active: AtomicBool::new(true),
            root_reconciliation_required: AtomicBool::new(false),
            state: Mutex::new(State::default()),
        });
        (
            Self {
                inner: inner.clone(),
            },
            InvocationWindow(inner),
        )
    }

    /// A pure child must not advance/drain unrelated protected-root events.
    /// An unexpected original is retained, but only the global supervisor may
    /// reconcile that root; metadata acknowledgement cannot clear this fence.
    pub fn requires_supervised_root_reconciliation(&self) -> bool {
        self.inner
            .root_reconciliation_required
            .load(Ordering::Acquire)
    }

    /// Exactly one sealed export. This does not reacquire the outer Recipe's
    /// execution mutex. Open uses its real live parent and therefore the worker's
    /// existing task compute account, cancellation and capacity controls.
    pub async fn invoke(
        &self,
        invocation: SealedPureInvocation,
    ) -> Result<PureInvocationObservation, RetainedExecutionError> {
        let mut state = self.inner.state.lock().await;
        state.stage = Some("invocation_window");
        if !self.inner.active.load(Ordering::Acquire) || state.started {
            return Err(invalid("Pure invocation window is closed or already used"));
        }
        state.started = true;
        #[cfg(any(test, feature = "test-support"))]
        {
            state.test_subject = Some((invocation.component.uuid, invocation.export.clone()));
        }
        state.stage = Some("typed_binding");
        let limits = self.inner.limits;
        if invocation.limits.max_depth != limits.max_depth
            || invocation.limits.max_nodes != limits.max_nodes
            || invocation.limits.max_bytes != limits.max_bytes
        {
            return Err(invalid("Pure invocation bounds differ from its HostCall"));
        }
        let inputs_checksum = Sha256::digest(
            serde_json::to_vec(&invocation.inputs)
                .map_err(|_| invalid("Pure inputs cannot be encoded"))?,
        )
        .into();
        let invocation_checksum = invocation.selected.checksum;
        let component = invocation.component;
        let export = invocation.export.clone();
        let observation = self.observe(&mut state, invocation).await;
        let primary = observation
            .as_ref()
            .err()
            .map(|error| failure_descriptor(&state, error));
        // Never return candidate data before positive release acknowledgement.
        // On interrupted waits, pending retains the original request; no new
        // invocation or implicit replay is possible through this carrier.
        let release = if let Some(context) = state.context {
            if state.pending.is_none() && state.received.is_none() {
                self.release(&mut state, context).await
            } else {
                Err(invalid("Pure invocation transport outcome is unresolved"))
            }
        } else {
            Ok(())
        };
        if let Some(primary) = primary {
            state.failure_facts = Some(serde_json::json!({"primary":primary,
                "cleanup":release.as_ref().err().map(|error|failure_descriptor(&state,error))}));
        } else if let Err(error) = &release {
            state.failure_facts = Some(
                serde_json::json!({"primary":failure_descriptor(&state,error),"cleanup":null}),
            );
        }
        let outcome = observation?;
        release?;
        Ok(PureInvocationObservation {
            format: "original-pure-invocation/1",
            component,
            export,
            task: self.inner.task,
            parent: self.inner.parent,
            host_key: self.inner.host_key,
            context: state
                .context
                .ok_or_else(|| invalid("Pure context missing after release"))?,
            inputs_checksum,
            invocation_checksum,
            outcome,
        })
    }

    /// Collect only an already submitted original. This cannot
    /// open, start, resume, release or replace a context, including after Drop
    /// closes the invocation window. Missing release remains unresolved.
    pub async fn collect_original_receipt(&self) -> Result<(), RetainedExecutionError> {
        let mut state = self.inner.state.lock().await;
        if state.received.is_none()
            && let Some(ticket) = state.pending.as_ref()
        {
            let receipt = ticket
                .wait()
                .await
                .map_err(|kind| RetainedExecutionError::Receipt {
                    request: ticket.id,
                    kind,
                })?;
            state.pending = None;
            state.received = Some(receipt);
        }
        // Keep the collected original for supervisor inspection. Collection
        // synthesizes neither a scheduling continuation nor a release.
        Ok(())
    }

    /// Fixed-size manifest: every internal scheduling receipt contributes to
    /// the checked rolling digest/count. Initial Start, terminal and release
    /// retain full originals; known pure ControlYield ticks are not Tool effects
    /// or durable checkpoints. Crash/unknown outcomes remain fenced by the one
    /// durable native invocation intent and never replay the case.
    pub async fn native_receipt_facts(&self) -> Result<Value, RetainedExecutionError> {
        let state = self.inner.state.lock().await;
        Ok(
            serde_json::json!({"format":"original-pure-native-receipts/2","task":self.inner.task,
            "parent":self.inner.parent,"host_key":self.inner.host_key,"root":self.inner.root,
            "started":state.started,"pending_request":state.pending.as_ref().map(|ticket|ticket.id.as_uuid()),
            "received_request":state.received.as_ref().map(|receipt|receipt.id.as_uuid()),
            "receipt_count":state.receipt_count,"receipt_chain":state.receipt_chain,
            "terminal_stage":state.terminal_stage,"retained_receipts":state.retained.iter().map(|(stage,r)|
                serde_json::json!({"stage":stage,"ordinal":r.ordinal,"request_id":r.request_id,"checksum":r.checksum})).collect::<Vec<_>>()}),
        )
    }

    /// Fixed Open/Preload/preload-complete/Start/terminal/release originals. Arc
    /// clones share immutable bytes; no per-yield payload list is exposed.
    pub async fn retained_native_receipts(&self) -> BTreeMap<&'static str, Arc<NativePureReceipt>> {
        self.inner.state.lock().await.retained.clone()
    }

    /// Failure-only original evidence. An unresolved received receipt remains
    /// in the shared carrier; this immutable encoded view grants no completion,
    /// release or replay authority. There is at most one such live receipt.
    pub async fn failure_native_receipts(
        &self,
    ) -> Result<BTreeMap<&'static str, Arc<NativePureReceipt>>, RetainedExecutionError> {
        let state = self.inner.state.lock().await;
        let mut originals = state.retained.clone();
        if let Some(receipt) = state.received.as_ref() {
            let command =
                receipt
                    .original_command()
                    .map_err(|kind| RetainedExecutionError::Receipt {
                        request: receipt.id,
                        kind,
                    })?;
            originals.insert(
                "unresolved",
                Arc::new(encode_receipt(
                    receipt,
                    &command,
                    state.receipt_count,
                    state.receipt_chain,
                    true,
                )?),
            );
        }
        Ok(originals)
    }
    pub async fn failure_facts(&self, error: &RetainedExecutionError) -> Value {
        let state = self.inner.state.lock().await;
        let mut facts = state.failure_facts.clone().unwrap_or_else(
            || serde_json::json!({"primary":failure_descriptor(&state,error),"cleanup":null}),
        );
        if let Some(object) = facts.as_object_mut() {
            object.insert(
                "supervised_root_reconciliation_required".into(),
                Value::Bool(self.requires_supervised_root_reconciliation()),
            );
        }
        facts
    }

    async fn observe(
        &self,
        state: &mut State,
        invocation: SealedPureInvocation,
    ) -> Result<PureCandidateOutcome, RetainedExecutionError> {
        let receipt = self
            .exchange(
                state,
                RecipeCommand::Open {
                    task: self.inner.task,
                    parent: Some(self.inner.parent),
                },
            )
            .await?;
        let snapshot = receipt.outcome.map_err(process_error)?;
        let context = match snapshot.recipe.as_ref() {
            Some(RecipeEvent::Opened { task, context }) if *task == self.inner.task => *context,
            _ => return Err(invalid("Pure worker did not open its same-task child")),
        };
        drop(snapshot);
        state.context = Some(context);
        let mut receipt = self
            .exchange(
                state,
                RecipeCommand::Preload {
                    context,
                    definitions: invocation.definitions,
                },
            )
            .await?;
        loop {
            let snapshot = receipt.outcome.map_err(process_error)?;
            match snapshot.recipe.as_ref() {
                Some(RecipeEvent::Progress {
                    context: actual,
                    boundary: RecipeBoundary::Complete { value },
                    ..
                }) if *actual == context && value.is_null() => break,
                Some(RecipeEvent::Progress {
                    context: actual,
                    boundary: RecipeBoundary::ControlYield { key },
                    ..
                }) if *actual == context => {
                    let key = *key;
                    receipt = self
                        .exchange(state, RecipeCommand::ResumeControl { context, key })
                        .await?;
                }
                _ => return Err(invalid("Pure preload did not complete without an effect")),
            }
        }
        let mut receipt = self
            .exchange(
                state,
                RecipeCommand::Start {
                    context,
                    selected: invocation.selected,
                    inputs: invocation.inputs,
                },
            )
            .await?;
        loop {
            // Decode the actual original before moving its non-cloneable
            // ProcessError into the retained error chain on technical failure.
            let original = if receipt.outcome.is_err() {
                Some(receipt.original_command().map_err(|kind| {
                    RetainedExecutionError::Receipt {
                        request: receipt.id,
                        kind,
                    }
                })?)
            } else {
                None
            };
            match receipt.outcome {
                Ok(snapshot) => match snapshot.recipe.as_ref() {
                    Some(RecipeEvent::Progress {
                        context: actual,
                        boundary: RecipeBoundary::Complete { value },
                        ..
                    }) if *actual == context => {
                        return Ok(
                            if invocation
                                .result_contract
                                .validate_with_limits(value, invocation.limits)
                                .is_ok()
                            {
                                PureCandidateOutcome::Return {
                                    value: value.clone(),
                                }
                            } else {
                                PureCandidateOutcome::ResultContractViolation {
                                    value: value.clone(),
                                }
                            },
                        );
                    }
                    Some(RecipeEvent::Progress {
                        context: actual,
                        boundary: RecipeBoundary::ControlYield { key },
                        ..
                    }) if *actual == context => {
                        let key = *key;
                        receipt = self
                            .exchange(state, RecipeCommand::ResumeControl { context, key })
                            .await?;
                    }
                    _ => {
                        return Err(invalid(
                            "Pure invocation reached an unexpected worker boundary",
                        ));
                    }
                },
                Err(error) => {
                    // Only an actual Python exception from this Start/control
                    // command and exact failed child is observed as candidate data.
                    // Resource, protocol, preload and root failures remain errors.
                    let exact_command = matches!(original,
                        Some(WorkerCommand::Recipe { command: RecipeCommand::Start { context: actual, .. }
                            | RecipeCommand::ResumeControl { context: actual, .. } }) if actual == context);
                    if error.kind == ProcessFailure::Vm(VmFailure::Python) && exact_command
                        && error.snapshot.as_ref().is_some_and(|snapshot|
                            snapshot.lifecycle == Lifecycle::Ready && matches!(&snapshot.recipe,
                                Some(RecipeEvent::Failed { context: Some(actual), .. }) if *actual == context))
                        && let Some(exception) = error.python_exception.as_ref().filter(|e| e.has_supported_format()) {
                        // Monty 1.0 can expose native heap/time/recursion limits
                        // through these Python exception classes without a
                        // separate resource-origin field. Their class alone
                        // cannot prove a candidate domain exception. Preserve
                        // the actual typed observation/original receipt and
                        // fail closed; never interpret a resource limit as a
                        // successfully observed domain outcome.
                        if matches!(exception.class(), "MemoryError" | "TimeoutError" | "RecursionError") {
                            return Err(process_error(error));
                        }
                        return Ok(PureCandidateOutcome::PythonException { exception: exception.clone() });
                    }
                    return Err(process_error(error));
                }
            }
        }
    }

    async fn release(
        &self,
        state: &mut State,
        context: RecipeContextId,
    ) -> Result<(), RetainedExecutionError> {
        let receipt = self
            .exchange(state, RecipeCommand::CancelContext { context })
            .await?;
        match receipt.outcome.map_err(process_error)?.recipe.as_ref() {
            Some(RecipeEvent::Released {
                task: None,
                contexts,
                parent_host: Some(parent),
            }) if parent.task == self.inner.task
                && parent.context == self.inner.parent
                && parent.key == self.inner.host_key
                && contexts.len() == 1
                && contexts[0].context == context
                && contexts[0].pending_host.is_none() =>
            {
                Ok(())
            }
            _ => Err(invalid("Pure child release was not acknowledged exactly")),
        }
    }

    async fn exchange(
        &self,
        state: &mut State,
        command: RecipeCommand,
    ) -> Result<TransportReceipt, RetainedExecutionError> {
        if state.pending.is_some()
            || state.received.is_some()
            || !self.inner.active.load(Ordering::Acquire)
        {
            return Err(invalid(
                "Pure command is fenced or has an unresolved predecessor",
            ));
        }
        state.stage = Some(match &command {
            RecipeCommand::Open { .. } => "open",
            RecipeCommand::Preload { .. } => "preload",
            RecipeCommand::Start { .. } => "start",
            RecipeCommand::ResumeControl { .. } if state.feeding_preload => "preload_control",
            RecipeCommand::ResumeControl { .. } => "invocation_control",
            RecipeCommand::CancelContext { .. } => "release",
            _ => "unsupported",
        });
        state.native_failure = None;
        let command = WorkerCommand::Recipe { command };
        state.expected_command_checksum = Some(
            Sha256::digest(
                serde_json::to_vec(&command)
                    .map_err(|_| invalid("Pure command encoding failed"))?,
            )
            .into(),
        );
        state.pending = Some(
            self.inner
                .transport
                .try_submit(command)
                .map_err(|error| RetainedExecutionError::Submission(Arc::new(error)))?,
        );
        let ticket = state.pending.as_ref().expect("stored original ticket");
        #[cfg(any(test, feature = "test-support"))]
        if let Some(subject) = &state.test_subject {
            testing::after_submit(subject, state.stage, &self.inner.transport, ticket.id).await?;
        }
        let receipt = ticket
            .wait()
            .await
            .map_err(|kind| RetainedExecutionError::Receipt {
                request: ticket.id,
                kind,
            })?;
        // wait() atomically collects and refunds the actual actor ledger credit.
        // Payload custody moves here before any further cancellation point.
        state.pending = None;
        state.native_failure = receipt.outcome.as_ref().err().map(|error| error.kind);
        state.received = Some(receipt);
        self.check_received(state)?;
        Ok(state
            .received
            .take()
            .expect("checked original native receipt"))
    }

    fn check_received(&self, state: &mut State) -> Result<(), RetainedExecutionError> {
        let receipt = state
            .received
            .as_ref()
            .ok_or_else(|| invalid("Pure receipt missing"))?;
        let original =
            receipt
                .original_command()
                .map_err(|kind| RetainedExecutionError::Receipt {
                    request: receipt.id,
                    kind,
                })?;
        let actual: [u8; 32] = Sha256::digest(
            serde_json::to_vec(&original)
                .map_err(|_| invalid("original Pure command encoding failed"))?,
        )
        .into();
        if state.expected_command_checksum != Some(actual) {
            return Err(invalid("Pure receipt command differs"));
        }
        let snapshot = match &receipt.outcome {
            Ok(snapshot) => Some(snapshot),
            Err(error) => error.snapshot.as_deref(),
        };
        if snapshot.is_none() {
            return Err(invalid(
                "Pure transport receipt has no confirmed worker snapshot",
            ));
        }
        if snapshot.is_some_and(|snapshot| {
            snapshot.boundary.is_some()
                || snapshot.admitted_task.is_some()
                || !snapshot.withheld_answers.is_empty()
                || !snapshot.stdout.is_empty()
        }) {
            self.inner
                .root_reconciliation_required
                .store(true, Ordering::Release);
            return Err(invalid(
                "Pure child returned unrelated protected-root events; supervised reconciliation required",
            ));
        }
        if snapshot.is_some_and(|snapshot| {
            snapshot.root != Some(self.inner.root)
                || snapshot.lifecycle != Lifecycle::Ready
                || snapshot
                    .task_accounting
                    .iter()
                    .filter(|a| a.task == self.inner.task)
                    .count()
                    != 1
                || snapshot
                    .task_accounting
                    .iter()
                    .find(|a| a.task == self.inner.task)
                    .is_none_or(|a| a.compute_time.is_none() || a.failure.is_some())
        }) {
            return Err(invalid(
                "Pure receipt lost its original live root or task account",
            ));
        }
        let next = state
            .receipt_count
            .checked_add(1)
            .ok_or_else(|| invalid("Pure receipt ordinal overflow"))?;
        let retain = match &original {
            WorkerCommand::Recipe {
                command: RecipeCommand::ResumeControl { .. },
            } => receipt.outcome.as_ref().ok().is_none_or(|snapshot| {
                !matches!(
                    snapshot.recipe,
                    Some(RecipeEvent::Progress {
                        boundary: RecipeBoundary::ControlYield { .. },
                        ..
                    })
                )
            }),
            _ => true,
        };
        let native = encode_receipt(
            receipt,
            &original,
            state.receipt_count,
            state.receipt_chain,
            retain,
        )?;
        let checksum = native.checksum;
        match &original {
            WorkerCommand::Recipe {
                command: RecipeCommand::Preload { .. },
            } => state.feeding_preload = true,
            WorkerCommand::Recipe {
                command: RecipeCommand::Start { .. },
            } => state.feeding_preload = false,
            _ => {}
        }
        let terminal = match &receipt.outcome {
            Ok(snapshot) => !matches!(
                snapshot.recipe,
                Some(RecipeEvent::Opened { .. })
                    | Some(RecipeEvent::Progress {
                        boundary: RecipeBoundary::ControlYield { .. },
                        ..
                    })
            ),
            Err(_) => true,
        };
        let stage = match &original {
            WorkerCommand::Recipe {
                command: RecipeCommand::Open { .. },
            } => Some("open"),
            WorkerCommand::Recipe {
                command: RecipeCommand::Preload { .. },
            } => Some("preload"),
            WorkerCommand::Recipe {
                command: RecipeCommand::Start { .. },
            } => Some("start"),
            WorkerCommand::Recipe {
                command: RecipeCommand::CancelContext { .. },
            } => Some("release"),
            _ if terminal && state.feeding_preload => Some("preload_terminal"),
            _ if terminal => Some("terminal"),
            _ => None,
        };
        if let Some(stage) = stage {
            if state.retained.insert(stage, Arc::new(native)).is_some() {
                return Err(invalid("Pure original receipt stage repeated"));
            }
            if terminal && !state.feeding_preload && stage != "release" {
                state.terminal_stage = Some(stage);
            }
        }
        state.receipt_count = next;
        state.receipt_chain = Some(checksum);
        Ok(())
    }
}

fn invalid(reason: &'static str) -> RetainedExecutionError {
    RetainedExecutionError::Invalid(reason)
}
fn process_error(error: brassclaw_monty_host::process::ProcessError) -> RetainedExecutionError {
    // Exact raw bytes are already retained by check_received. Also preserve
    // the non-cloneable native error and its private IO causes through the
    // caller's existing failure-custody chain. ProcessError's Display/Debug
    // expose only its classification, never candidate data or diagnostics.
    RetainedExecutionError::Process(Arc::new(error))
}

fn encode_receipt(
    receipt: &TransportReceipt,
    command: &WorkerCommand,
    ordinal: u64,
    previous: Option<[u8; 32]>,
    retain: bool,
) -> Result<NativePureReceipt, RetainedExecutionError> {
    use brassclaw_monty_host::process::{ProcessSnapshot, PythonExceptionObservation};
    #[derive(Serialize)]
    struct Record<'a> {
        format: &'static str,
        ordinal: u64,
        previous_checksum: Option<[u8; 32]>,
        request_id: uuid::Uuid,
        transport_started: bool,
        response_timeout: std::time::Duration,
        max_frame_bytes: usize,
        failure: Option<String>,
        exit_status: Option<String>,
        containment_error: Option<String>,
        reap_error: Option<String>,
        command: Option<&'a WorkerCommand>,
        snapshot: Option<&'a ProcessSnapshot>,
        diagnostic: Option<&'a str>,
        python_exception: Option<&'a PythonExceptionObservation>,
    }
    let (snapshot, error) = match &receipt.outcome {
        Ok(snapshot) => (Some(snapshot), None),
        Err(error) => (error.snapshot.as_deref(), Some(error)),
    };
    let mut record = Record {
        format: "original-pure-native-receipt/1",
        ordinal,
        previous_checksum: previous,
        request_id: receipt.id.as_uuid(),
        transport_started: receipt.transport_started,
        response_timeout: receipt.response_timeout,
        max_frame_bytes: receipt.max_frame_bytes,
        failure: error.map(|e| format!("{:?}", e.kind)),
        exit_status: error.and_then(|e| e.exit_status).map(|s| s.to_string()),
        containment_error: error
            .and_then(|e| e.containment_error.as_ref())
            .map(ToString::to_string),
        reap_error: error
            .and_then(|e| e.reap_error.as_ref())
            .map(ToString::to_string),
        command: None,
        snapshot: None,
        diagnostic: None,
        python_exception: None,
    };
    // Both original IPC frames already obey this actual request's captured
    // frame credit. Add only the measured receipt-envelope/parent-error bytes;
    // there is no new fixed byte quota or unbounded growing receipt collection.
    let metadata = brassclaw_common::json::count_json_bytes(&record, None)
        .map_err(|_| invalid("Pure receipt metadata encoding failed"))?;
    let limit = u64::try_from(receipt.max_frame_bytes)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(metadata))
        .ok_or_else(|| invalid("Pure receipt frame accounting overflow"))?;
    record.command = Some(command);
    record.snapshot = snapshot;
    record.diagnostic = error.and_then(|e| e.diagnostic.as_deref());
    record.python_exception = error.and_then(|e| e.python_exception.as_ref());
    let count = brassclaw_common::json::count_json_bytes(&record, Some(limit))
        .map_err(|_| invalid("Pure receipt exceeds its actual IPC-frame accounting"))?;
    let count = usize::try_from(count)
        .map_err(|_| invalid("Pure receipt size exceeds host representation"))?;
    struct HashWriter(Sha256);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (bytes, checksum) = if retain {
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(count)
            .map_err(|_| invalid("Pure receipt host allocation failed"))?;
        brassclaw_common::json::write_json(&mut encoded, &record)
            .map_err(|_| invalid("Pure receipt encoding failed"))?;
        let checksum = Sha256::digest(&encoded).into();
        (
            String::from_utf8(encoded)
                .map_err(|_| invalid("Pure receipt encoding is not UTF-8"))?,
            checksum,
        )
    } else {
        let mut hash = HashWriter(Sha256::new());
        brassclaw_common::json::write_json(&mut hash, &record)
            .map_err(|_| invalid("Pure receipt digest failed"))?;
        (String::new(), hash.0.finalize().into())
    };
    Ok(NativePureReceipt {
        ordinal,
        request_id: receipt.id.as_uuid(),
        previous,
        bytes,
        checksum,
    })
}

fn failure_descriptor(state: &State, error: &RetainedExecutionError) -> Value {
    let cause = match error {
        RetainedExecutionError::Invalid(reason) => {
            serde_json::json!({"kind":"boundary","reason":reason})
        }
        RetainedExecutionError::Inputs(_) => serde_json::json!({"kind":"input_contract"}),
        RetainedExecutionError::Tool(_) => serde_json::json!({"kind":"tool_contract"}),
        RetainedExecutionError::Submission(error) => {
            serde_json::json!({"kind":"submission","actor_kind":format!("{:?}",error.kind)})
        }
        RetainedExecutionError::Receipt { request, kind } => {
            serde_json::json!({"kind":"receipt","request_id":request.as_uuid(),"actor_kind":format!("{kind:?}")})
        }
        RetainedExecutionError::Process(error) => {
            serde_json::json!({"kind":"process","failure":native_failure_kind(error.kind)})
        }
    };
    serde_json::json!({"stage":state.stage,"cause":cause,"native_failure":state.native_failure.map(native_failure_kind),
        "pending_request":state.pending.as_ref().map(|t|t.id.as_uuid()),
        "received_request":state.received.as_ref().map(|r|r.id.as_uuid()),
        "expected_command_checksum":state.expected_command_checksum})
}
fn native_failure_kind(kind: ProcessFailure) -> Value {
    match kind {
        ProcessFailure::Vm(failure) => serde_json::json!({"kind":"vm","failure":failure}),
        kind => serde_json::json!({"kind":"transport","failure":format!("{kind:?}")}),
    }
}

/// Real-worker acceptance controls. No fabricated result, receipt, failure or
/// authority can be supplied through this module; ordinary builds omit it.
#[cfg(any(test, feature = "test-support"))]
pub mod testing {
    use super::*;
    use std::sync::{Mutex as StdMutex, OnceLock};
    use tokio::sync::Notify;
    #[derive(Clone, Copy, Debug)]
    pub struct Observation {
        pub request_id: uuid::Uuid,
        pub transport_started: bool,
        pub received: bool,
    }
    pub struct CommandPause {
        pub reached: Notify,
        pub release: Notify,
        observed: OnceLock<Observation>,
        wait_for_receipt: bool,
    }
    impl CommandPause {
        pub fn observation(&self) -> Option<Observation> {
            self.observed.get().copied()
        }
    }
    type Key = (uuid::Uuid, String, &'static str);
    type Pauses = std::collections::BTreeMap<Key, Arc<CommandPause>>;
    static PAUSES: OnceLock<StdMutex<Pauses>> = OnceLock::new();
    /// Select a single already-declared component/export/command boundary.
    /// Selection removes the gate before waiting, so another case cannot reuse
    /// it. The test must perform any actual failure/cancellation itself.
    pub fn pause_next(
        component: uuid::Uuid,
        export: &str,
        stage: &'static str,
        wait_for_receipt: bool,
    ) -> Arc<CommandPause> {
        assert!(matches!(stage, "start" | "release"));
        let pause = Arc::new(CommandPause {
            reached: Notify::new(),
            release: Notify::new(),
            observed: OnceLock::new(),
            wait_for_receipt,
        });
        assert!(
            PAUSES
                .get_or_init(StdMutex::default)
                .lock()
                .unwrap()
                .insert((component, export.to_owned(), stage), pause.clone())
                .is_none()
        );
        pause
    }
    pub(super) async fn after_submit(
        subject: &(uuid::Uuid, String),
        stage: Option<&'static str>,
        transport: &TransportClient,
        id: brassclaw_monty_host::transport_actor::RequestId,
    ) -> Result<(), RetainedExecutionError> {
        let Some(stage) = stage else { return Ok(()) };
        let pause = PAUSES
            .get_or_init(StdMutex::default)
            .lock()
            .map_err(|_| invalid("original command acceptance gate unavailable"))?
            .remove(&(subject.0, subject.1.clone(), stage));
        let Some(pause) = pause else { return Ok(()) };
        let inbox = transport.completions();
        if pause.wait_for_receipt {
            inbox
                .ready(id)
                .await
                .map_err(|kind| RetainedExecutionError::Receipt { request: id, kind })?;
        }
        let (started, received) = loop {
            let state = inbox
                .request_observation(id)
                .map_err(|kind| RetainedExecutionError::Receipt { request: id, kind })?;
            if state.0 || state.1 {
                break state;
            }
            tokio::task::yield_now().await;
        };
        let _ = pause.observed.set(Observation {
            request_id: id.as_uuid(),
            transport_started: started,
            received,
        });
        pause.reached.notify_one();
        pause.release.notified().await;
        Ok(())
    }
}
