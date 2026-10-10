//! Executes only the step requested by Monty, using the already retained IBS
//! program and one task-owned child context. It never selects the next step,
//! looks up latest, retries an effect or turns a Recipe failure into No-Match.
//! The catalogue owner must establish approval and retain actual Tool adapters
//! before exposing this primitive to production. Preparation alone is no grant.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use brassclaw_monty_host::{
    process::{
        PortAnswer, ProcessError, ProcessSnapshot, RecipeBoundary, RecipeCommand, RecipeContextId,
        RecipeEvent, SelectedPython, TaskHandle, WorkerCommand,
    },
    transport_actor::{
        ActorFailure, RequestId, RequestTicket, SubmitError, TransportClient, TransportReceipt,
    },
};
use brassclaw_skills::{
    association_contract::FailureAction,
    component_revision::REVISION_LIMITS,
    value_contract::{ContractError, validate_data_bounds},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::memory::{
    composition::ComposedProgram,
    retained_inputs::{RetainedInputError, RetainedRecipeInputs, RetainedUnboundProgram},
    retained_tools::{RetainedToolBinding, RetainedToolError, RetainedToolProgram},
};

// Loading definitions has no Tool binding/port. Invocation supplies the exact
// retained usage together; the two modes cannot be confused by boolean arguments.
enum StepFeedMode<'a> {
    Preload,
    Execute {
        binding: Option<&'a RetainedToolBinding>,
        tools: Option<&'a dyn RetainedToolPort>,
    },
}

#[derive(Clone)]
pub enum RetainedProgram {
    Unbound(Arc<RetainedUnboundProgram>),
    Tools(Arc<RetainedToolProgram>),
}
impl RetainedProgram {
    pub fn inputs(&self) -> &RetainedRecipeInputs {
        match self {
            Self::Unbound(p) => p.inputs(),
            Self::Tools(p) => p.inputs(),
        }
    }
    pub fn program(&self) -> &ComposedProgram {
        match self {
            Self::Unbound(p) => p.program(),
            Self::Tools(p) => p.program(),
        }
    }
    pub(super) fn binding(&self, step: &str) -> Option<&RetainedToolBinding> {
        match self {
            Self::Unbound(_) => None,
            Self::Tools(p) => p.bindings().get(step),
        }
    }
}

/// Exact task and selected step supplied by this executor at the observed child
/// HostCall. An address is neither Tool permission nor a retry/deduplication key.
pub struct RetainedToolInvocation<'a> {
    task: TaskHandle,
    step_id: &'a str,
}
impl RetainedToolInvocation<'_> {
    pub fn task(&self) -> TaskHandle {
        self.task
    }
    pub fn step_id(&self) -> &str {
        self.step_id
    }
}

#[async_trait]
/// The trusted caller retains the exact implementation and dispatches through
/// its real kernel, current policy and durable effect adapter. The instance
/// service must own this future to actual completion even after cancellation.
pub trait RetainedToolPort: Send + Sync {
    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer;
}

#[derive(Debug, thiserror::Error)]
pub enum RetainedExecutionError {
    #[error("retained execution: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Inputs(#[from] RetainedInputError),
    #[error(transparent)]
    Tool(#[from] RetainedToolError),
    #[error("retained worker submission failed")]
    Submission(#[source] Arc<SubmitError>),
    #[error("retained worker receipt unavailable for {request:?}: {kind}")]
    Receipt {
        request: RequestId,
        kind: ActorFailure,
    },
    #[error(transparent)]
    Process(Arc<ProcessError>),
}

/// Full private transport evidence survives conversion to a classified task
/// port failure. Await failures retain the ledger identity; the instance actor
/// continues owning any command whose actual outcome has not yet been collected.
pub enum RetainedTransportEvidence {
    Submission(Arc<SubmitError>),
    Receipt {
        request: RequestId,
        kind: ActorFailure,
    },
    Process {
        error: Arc<ProcessError>,
        request: RequestId,
        command: Result<Box<WorkerCommand>, ActorFailure>,
    },
}

enum TaskContextState {
    Empty,
    Rejected(Arc<SubmitError>),
    Pending(RequestTicket),
    Settled(Box<TransportReceipt>),
}

/// One task-owned parent for explicitly delegated Recipe children. Opening
/// this idle context executes no Python or Tool. The actual request/receipt is
/// retained before/after awaits, so abandonment cannot cause a second root
/// allocation or discard an uncertain worker outcome.
pub struct RetainedTaskContext {
    task: TaskHandle,
    state: TaskContextState,
}
impl RetainedTaskContext {
    pub fn new(task: TaskHandle) -> Self {
        Self {
            task,
            state: TaskContextState::Empty,
        }
    }

    pub fn task(&self) -> TaskHandle {
        self.task
    }

    /// Actual transport evidence for supervisor reconciliation, never permission
    /// to replay a task, reopen a child or restore another attempt's state.
    pub fn receipt(&self) -> Option<&TransportReceipt> {
        match &self.state {
            TaskContextState::Settled(receipt) => Some(receipt),
            _ => None,
        }
    }

    pub async fn open(
        &mut self,
        transport: &TransportClient,
    ) -> Result<RecipeContextId, RetainedExecutionError> {
        if matches!(self.state, TaskContextState::Empty) {
            self.state = match transport.try_submit(WorkerCommand::Recipe {
                command: RecipeCommand::Open {
                    task: self.task,
                    parent: None,
                },
            }) {
                Ok(ticket) => TaskContextState::Pending(ticket),
                Err(error) => TaskContextState::Rejected(Arc::new(error)),
            };
        }
        if let TaskContextState::Pending(ticket) = &self.state {
            let receipt = ticket
                .wait()
                .await
                .map_err(|kind| RetainedExecutionError::Receipt {
                    request: ticket.id,
                    kind,
                })?;
            self.state = TaskContextState::Settled(Box::new(receipt));
        }
        match &self.state {
            TaskContextState::Rejected(error) => {
                Err(RetainedExecutionError::Submission(error.clone()))
            }
            TaskContextState::Settled(receipt) => match &receipt.outcome {
                Ok(snapshot) => match &snapshot.recipe {
                    Some(RecipeEvent::Opened { task, context }) if *task == self.task => {
                        Ok(*context)
                    }
                    _ => Err(RetainedExecutionError::Invalid(
                        "worker did not open the task parent context",
                    )),
                },
                Err(_) => Err(RetainedExecutionError::Invalid(
                    "task parent context failed; retained receipt requires reconciliation",
                )),
            },
            _ => Err(RetainedExecutionError::Invalid(
                "task parent context has no observed receipt",
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Failed,
}

/// Classified execution outcome, without private transport/authoring details.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RetainedStepFailure {
    ResultContract,
    ToolContract,
    Submission,
    Receipt,
    Process,
    Execution,
}
impl RetainedStepFailure {
    pub fn reason_kind(self) -> &'static str {
        match self {
            Self::ResultContract => "result_contract_failed",
            Self::ToolContract => "tool_contract_failed",
            Self::Submission => "worker_submission_failed",
            Self::Receipt => "worker_receipt_unavailable",
            Self::Process => "worker_execution_failed",
            Self::Execution => "recipe_execution_failed",
        }
    }
}

/// Actual execution observations. Private fields prevent a caller-supplied
/// success flag from becoming evidence. Fingerprints use typed-value-sha256/1;
/// full Tool arguments/answers remain in the durable invocation journal.
pub struct RetainedStepObservation {
    input_checksum: [u8; 32],
    arguments_checksum: Option<[u8; 32]>,
    answer_checksum: Option<[u8; 32]>,
    result_checksum: Option<[u8; 32]>,
    failure: Option<RetainedStepFailure>,
    settled: bool,
    observation_error: Option<ContractError>,
}
impl RetainedStepObservation {
    pub fn input_checksum(&self) -> [u8; 32] {
        self.input_checksum
    }
    pub fn arguments_checksum(&self) -> Option<[u8; 32]> {
        self.arguments_checksum
    }
    pub fn answer_checksum(&self) -> Option<[u8; 32]> {
        self.answer_checksum
    }
    pub fn result_checksum(&self) -> Option<[u8; 32]> {
        self.result_checksum
    }
    pub fn failure(&self) -> Option<RetainedStepFailure> {
        self.failure
    }
    pub fn settled(&self) -> bool {
        self.settled
    }
    pub fn observation_error(&self) -> Option<&ContractError> {
        self.observation_error.as_ref()
    }
}

/// Domain-separated fingerprint of the exact typed value, independent of JSON
/// object insertion order. Numbers retain their transport representation; no
/// numeric coercion, source interpretation or raw-value diagnostic is involved.
pub fn typed_value_checksum(value: &Value) -> Result<[u8; 32], ContractError> {
    validate_data_bounds(value, REVISION_LIMITS)?;
    fn bytes(hash: &mut Sha256, value: &[u8]) {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value);
    }
    fn walk(hash: &mut Sha256, value: &Value) {
        match value {
            Value::Null => hash.update([0]),
            Value::Bool(value) => hash.update([1, u8::from(*value)]),
            Value::Number(value) => {
                hash.update([2]);
                bytes(hash, value.to_string().as_bytes());
            }
            Value::String(value) => {
                hash.update([3]);
                bytes(hash, value.as_bytes());
            }
            Value::Array(values) => {
                hash.update([4]);
                hash.update((values.len() as u64).to_be_bytes());
                for value in values {
                    walk(hash, value);
                }
            }
            Value::Object(values) => {
                hash.update([5]);
                hash.update((values.len() as u64).to_be_bytes());
                let mut keys: Vec<_> = values.keys().collect();
                keys.sort_unstable();
                for key in keys {
                    bytes(hash, key.as_bytes());
                    walk(hash, &values[key]);
                }
            }
        }
    }
    let mut hash = Sha256::new();
    hash.update(b"typed-value-sha256/1\0");
    walk(&mut hash, value);
    Ok(hash.finalize().into())
}

/// Fingerprint the actual answer variant as well as its contents. A returned
/// error-looking object is distinct from a classified Domain/Terminal error.
pub fn port_answer_checksum(answer: &PortAnswer) -> Result<[u8; 32], ContractError> {
    let mut hash = Sha256::new();
    hash.update(b"port-answer-sha256/1\0");
    match answer {
        PortAnswer::Return { value } => {
            hash.update([0]);
            hash.update(typed_value_checksum(value)?);
        }
        PortAnswer::DomainError { reason_kind } => {
            hash.update([1]);
            hash.update(reason_kind.as_bytes());
        }
        PortAnswer::TerminalError { reason_kind } => {
            hash.update([2]);
            hash.update(reason_kind.as_bytes());
        }
    }
    Ok(hash.finalize().into())
}

/// Private task state; never serialize its routing handles or Tool evidence
/// into model context. Results handed back to Monty are typed data. A dropped
/// step future leaves this execution fenced in Running; an error leaves Failed.
/// Neither permits another feed or implicit replay. The supervisor retains the
/// transport ledger and releases the task's actual contexts during settlement.
pub struct RetainedRecipeExecution {
    task: TaskHandle,
    program: RetainedProgram,
    source_checks: Arc<super::retained_source::InspectedRetainedProgram>,
    context: Option<RecipeContextId>,
    parent: Option<RecipeContextId>,
    phase: Phase,
    started: BTreeSet<String>,
    latest_snapshot: Option<ProcessSnapshot>,
    host_answers: Vec<(String, PortAnswer)>,
    observations: BTreeMap<String, RetainedStepObservation>,
    observation_order: Vec<String>,
    observe_behavior: bool,
    /// Completed prefix of the immutable dependency-first library selection.
    /// Advance only after the worker acknowledges an effect-free preload.
    preloaded: usize,
    transport_failure: Option<RetainedTransportEvidence>,
}
impl RetainedRecipeExecution {
    pub fn new(
        task: TaskHandle,
        inspected: Arc<super::retained_source::InspectedRetainedProgram>,
    ) -> Result<Self, RetainedExecutionError> {
        Self::prepare(task, inspected, false, None)
    }

    /// Execute beneath the actual same-task parent opened by Monty's transport
    /// owner. The worker verifies ownership and liveness. Inputs/results still
    /// travel explicitly as typed data; a parent link shares no implicit state.
    pub fn new_child(
        task: TaskHandle,
        parent: RecipeContextId,
        inspected: Arc<super::retained_source::InspectedRetainedProgram>,
    ) -> Result<Self, RetainedExecutionError> {
        Self::prepare(task, inspected, false, Some(parent))
    }

    /// The same actual runner/child/Tool path, with bounded observations enabled
    /// for behavioral review. Ordinary execution avoids extra data fingerprinting.
    /// Enabling observation supplies neither successful results nor approval.
    pub fn new_for_behavioral_validation(
        task: TaskHandle,
        inspected: Arc<super::retained_source::InspectedRetainedProgram>,
    ) -> Result<Self, RetainedExecutionError> {
        Self::prepare(task, inspected, true, None)
    }

    fn prepare(
        task: TaskHandle,
        inspected: Arc<super::retained_source::InspectedRetainedProgram>,
        observe_behavior: bool,
        parent: Option<RecipeContextId>,
    ) -> Result<Self, RetainedExecutionError> {
        let program = inspected.program().clone();
        // Check the supported complete layout before any VM or effect begins.
        program.inputs().monty_flow()?;
        if let RetainedProgram::Tools(p) = &program
            && p.bindings().values().any(|b| {
                b.association().failure().action() != FailureAction::Stop
                    || b.association().failure().max_attempts() != 1
            })
        {
            return Err(RetainedExecutionError::Invalid(
                "retry requires a durable invocation/evidence adapter",
            ));
        }
        Ok(Self {
            task,
            program,
            source_checks: inspected,
            context: None,
            parent,
            phase: Phase::Idle,
            started: BTreeSet::new(),
            latest_snapshot: None,
            host_answers: Vec::new(),
            observations: BTreeMap::new(),
            observation_order: Vec::new(),
            observe_behavior,
            preloaded: 0,
            transport_failure: None,
        })
    }

    pub fn latest_snapshot(&self) -> Option<&ProcessSnapshot> {
        self.latest_snapshot.as_ref()
    }
    /// Exact syntax observations retained before the first executable feed.
    /// These do not establish semantic review, activation or Tool authority.
    pub fn source_checks(&self) -> &super::retained_source::InspectedSources {
        self.source_checks.source_checks()
    }
    /// Actual selection retained by this executor, not a supplied review target.
    pub fn inspected_program(&self) -> Arc<super::retained_source::InspectedRetainedProgram> {
        self.source_checks.clone()
    }
    pub fn observations(&self) -> &BTreeMap<String, RetainedStepObservation> {
        &self.observations
    }
    /// Actual begun-step order, present only when behavioral observation was
    /// enabled before execution. A sorted map cannot establish workflow order.
    pub fn observation_order(&self) -> Option<&[String]> {
        self.observe_behavior.then_some(&self.observation_order)
    }
    /// Actual completed host answers survive a subsequent validation/IPC error.
    /// They are evidence, never permission to repeat a completed invocation.
    pub fn host_answers(&self) -> &[(String, PortAnswer)] {
        &self.host_answers
    }
    pub fn transport_failure(&self) -> Option<&RetainedTransportEvidence> {
        self.transport_failure.as_ref()
    }

    /// Confirmed successful prefix of the supported flat, stop-only workflow.
    /// A begun failed/in-flight feed is not a completed step. This inexpensive
    /// lifecycle evidence is independent of behavioral value fingerprinting.
    pub fn completed_step_ids(&self) -> impl Iterator<Item = &str> {
        let count = self
            .started
            .len()
            .saturating_sub(usize::from(self.phase != Phase::Idle));
        self.program
            .program()
            .steplist
            .iter()
            .take(count)
            .map(|step| step.step_id.as_str())
    }

    /// Only actual settled feeds can complete the selected workflow. Neither a
    /// published reply nor release of a child establishes this condition.
    pub fn is_complete(&self) -> bool {
        self.phase == Phase::Idle
            && !self.program.program().steplist.is_empty()
            && self.started.len() == self.program.program().steplist.len()
    }

    pub fn pending_step_id(&self) -> Option<&str> {
        (self.phase == Phase::Running).then(|| {
            self.program.program().steplist[self.started.len() - 1]
                .step_id
                .as_str()
        })
    }

    pub fn failed_step_id(&self) -> Option<&str> {
        (self.phase == Phase::Failed).then(|| {
            self.program.program().steplist[self.started.len() - 1]
                .step_id
                .as_str()
        })
    }

    async fn exchange(
        &mut self,
        transport: &TransportClient,
        command: RecipeCommand,
    ) -> Result<(), RetainedExecutionError> {
        let ticket = match transport.try_submit(WorkerCommand::Recipe { command }) {
            Ok(ticket) => ticket,
            Err(error) => {
                let error = Arc::new(error);
                self.transport_failure = Some(RetainedTransportEvidence::Submission(error.clone()));
                return Err(RetainedExecutionError::Submission(error));
            }
        };
        let receipt = match ticket.wait().await {
            Ok(receipt) => receipt,
            Err(kind) => {
                self.transport_failure = Some(RetainedTransportEvidence::Receipt {
                    request: ticket.id,
                    kind,
                });
                return Err(RetainedExecutionError::Receipt {
                    request: ticket.id,
                    kind,
                });
            }
        };
        // A worker-reported VM error need not repeat the submitted command in
        // ProcessError. Preserve its actual actor-ledger origin separately;
        // failed command recovery is evidence too, never silently discarded.
        let origin = receipt
            .outcome
            .as_ref()
            .err()
            .map(|_| receipt.original_command());
        match receipt.outcome {
            Ok(snapshot) => self.latest_snapshot = Some(snapshot),
            Err(error) => {
                let error = Arc::new(error);
                self.transport_failure = Some(RetainedTransportEvidence::Process {
                    error: error.clone(),
                    request: receipt.id,
                    command: origin.expect("observed failed receipt").map(Box::new),
                });
                return Err(RetainedExecutionError::Process(error));
            }
        }
        Ok(())
    }

    /// Execute exactly one requested component. Monty owns flow order and the
    /// inputs/result handoff; this loop handles only mechanical child boundaries.
    pub async fn run_step(
        &mut self,
        transport: &TransportClient,
        step_id: &str,
        supplied: &Value,
        tools: Option<&dyn RetainedToolPort>,
    ) -> Result<Value, RetainedExecutionError> {
        if self.phase != Phase::Idle || self.started.contains(step_id) {
            return Err(RetainedExecutionError::Invalid(
                "execution is fenced or step already began",
            ));
        }
        // IBS currently accepts only this exact flat stop-only flow. Check its
        // requested occurrence before any child allocation or Tool intent. Rust
        // verifies Monty's request; it never selects or executes the next step.
        if self
            .program
            .program()
            .steplist
            .get(self.started.len())
            .map(|step| step.step_id.as_str())
            != Some(step_id)
        {
            return Err(RetainedExecutionError::Invalid(
                "step is not the next retained occurrence",
            ));
        }
        let selected = self.program.clone();
        let step = selected
            .program()
            .steplist
            .iter()
            .find(|s| s.step_id == step_id)
            .ok_or(RetainedExecutionError::Invalid(
                "step is not in retained program",
            ))?;
        let inputs = selected.inputs().bind_step_inputs(step_id, supplied)?;
        let binding = selected.binding(step_id);
        if binding.is_some() && tools.is_none() {
            return Err(RetainedExecutionError::Invalid(
                "retained Tool implementation required",
            ));
        }
        let aliases = binding
            .map(|b| {
                // The association parser already checked the host.<identifier> grammar.
                BTreeSet::from([b.association().callable()[5..].to_owned()])
            })
            .unwrap_or_default();
        let source = self
            .source_checks
            .invocation(step_id)
            .unwrap_or(&step.executable_code);
        let python = SelectedPython {
            source: source.to_owned(),
            checksum: Sha256::digest(source.as_bytes()).into(),
            aliases,
        };
        let input_checksum = self
            .observe_behavior
            .then(|| typed_value_checksum(&inputs))
            .transpose()
            .map_err(RetainedInputError::from)?;
        self.phase = Phase::Running;
        self.started.insert(step_id.to_owned());
        if let Some(input_checksum) = input_checksum {
            self.observation_order.push(step_id.to_owned());
            self.observations.insert(
                step_id.to_owned(),
                RetainedStepObservation {
                    input_checksum,
                    arguments_checksum: None,
                    answer_checksum: None,
                    result_checksum: None,
                    failure: None,
                    settled: false,
                    observation_error: None,
                },
            );
        }
        let result = async {
            while let Some(id) = self
                .source_checks
                .preload_order()
                .get(self.preloaded)
                .copied()
            {
                let source = selected.inputs().instruction().snapshot().revisions()[&id]
                    .draft()
                    .document()["content"]
                    .as_str()
                    .ok_or(RetainedExecutionError::Invalid(
                        "qualified preload source missing",
                    ))?;
                let source = format!("{source}\nresult = None");
                let preload = SelectedPython {
                    checksum: Sha256::digest(source.as_bytes()).into(),
                    source,
                    aliases: BTreeSet::new(),
                };
                self.feed(
                    transport,
                    step_id,
                    serde_json::json!({}),
                    preload,
                    StepFeedMode::Preload,
                )
                .await?;
                self.preloaded += 1;
            }
            self.feed(
                transport,
                step_id,
                inputs,
                python,
                StepFeedMode::Execute { binding, tools },
            )
            .await
        }
        .await;
        if let Some(observation) = self.observations.get_mut(step_id) {
            observation.settled = true;
            match &result {
                Ok(value) => match typed_value_checksum(value) {
                    Ok(checksum) => observation.result_checksum = Some(checksum),
                    Err(error) => observation.observation_error = Some(error),
                },
                Err(error) => {
                    observation.failure = Some(match error {
                        RetainedExecutionError::Inputs(_) => RetainedStepFailure::ResultContract,
                        RetainedExecutionError::Tool(_) => RetainedStepFailure::ToolContract,
                        RetainedExecutionError::Submission(_) => RetainedStepFailure::Submission,
                        RetainedExecutionError::Receipt { .. } => RetainedStepFailure::Receipt,
                        RetainedExecutionError::Process(_) => RetainedStepFailure::Process,
                        RetainedExecutionError::Invalid(_) => RetainedStepFailure::Execution,
                    })
                }
            }
        }
        self.phase = if result.is_ok() {
            Phase::Idle
        } else {
            Phase::Failed
        };
        result
    }

    async fn feed(
        &mut self,
        transport: &TransportClient,
        step_id: &str,
        inputs: Value,
        python: SelectedPython,
        mode: StepFeedMode<'_>,
    ) -> Result<Value, RetainedExecutionError> {
        let (binding, tools, preloading) = match mode {
            StepFeedMode::Preload => (None, None, true),
            StepFeedMode::Execute { binding, tools } => (binding, tools, false),
        };
        if self.context.is_none() {
            self.exchange(
                transport,
                RecipeCommand::Open {
                    task: self.task,
                    parent: self.parent,
                },
            )
            .await?;
            match self
                .latest_snapshot
                .as_ref()
                .and_then(|s| s.recipe.as_ref())
            {
                Some(RecipeEvent::Opened { task, context }) if *task == self.task => {
                    self.context = Some(*context)
                }
                _ => {
                    return Err(RetainedExecutionError::Invalid(
                        "worker did not open the admitted task context",
                    ));
                }
            }
        }
        let context = self
            .context
            .ok_or(RetainedExecutionError::Invalid("missing child context"))?;
        self.exchange(
            transport,
            RecipeCommand::Start {
                context,
                selected: python,
                inputs: inputs.clone(),
            },
        )
        .await?;
        let mut dispatched = false;
        loop {
            let event = self
                .latest_snapshot
                .as_mut()
                .and_then(|s| s.recipe.take())
                .ok_or(RetainedExecutionError::Invalid("missing child boundary"))?;
            match event {
                RecipeEvent::Progress {
                    context: actual,
                    boundary,
                    stdout,
                } if actual == context => {
                    match boundary {
                        RecipeBoundary::Complete { value } => {
                            self.latest_snapshot
                                .as_mut()
                                .expect("observed snapshot")
                                .recipe = Some(RecipeEvent::Progress {
                                context,
                                boundary: RecipeBoundary::Complete {
                                    value: value.clone(),
                                },
                                stdout,
                            });
                            if binding.is_some() && !dispatched {
                                return Err(RetainedExecutionError::Invalid(
                                    "selected Tool usage did not invoke its binding",
                                ));
                            }
                            if preloading {
                                if !value.is_null() {
                                    return Err(RetainedExecutionError::Invalid(
                                        "preload returned data",
                                    ));
                                }
                            } else {
                                self.program
                                    .inputs()
                                    .validate_step_result(step_id, &value)?;
                            }
                            return Ok(value);
                        }
                        RecipeBoundary::ControlYield { key } => {
                            self.latest_snapshot
                                .as_mut()
                                .expect("observed snapshot")
                                .recipe = Some(RecipeEvent::Progress {
                                context,
                                boundary: RecipeBoundary::ControlYield { key },
                                stdout,
                            });
                            self.exchange(transport, RecipeCommand::ResumeControl { context, key })
                                .await?;
                        }
                        RecipeBoundary::HostCall {
                            key,
                            name,
                            args,
                            kwargs,
                        } => {
                            // Preserve the real pending call if preparation/dispatch fails.
                            let arguments = Value::Object(kwargs.clone().into_iter().collect());
                            self.latest_snapshot
                                .as_mut()
                                .expect("observed snapshot")
                                .recipe = Some(RecipeEvent::Progress {
                                context,
                                boundary: RecipeBoundary::HostCall {
                                    key,
                                    name: name.clone(),
                                    args: args.clone(),
                                    kwargs,
                                },
                                stdout,
                            });
                            let binding = binding
                                .filter(|b| {
                                    !dispatched
                                        && args.is_empty()
                                        && b.association().callable()[5..] == name
                                })
                                .ok_or(RetainedExecutionError::Invalid(
                                    "unbound or repeated Tool call",
                                ))?;
                            let arguments = binding.bind_tool_arguments(&inputs, &arguments)?;
                            let port = tools.ok_or(RetainedExecutionError::Invalid(
                                "missing retained Tool port",
                            ))?;
                            if let Some(observation) = self.observations.get_mut(step_id) {
                                match typed_value_checksum(&arguments) {
                                    Ok(checksum) => observation.arguments_checksum = Some(checksum),
                                    Err(error) => observation.observation_error = Some(error),
                                }
                            }
                            let answer = port
                                .dispatch(
                                    RetainedToolInvocation {
                                        task: self.task,
                                        step_id,
                                    },
                                    binding,
                                    arguments,
                                )
                                .await;
                            if let Some(observation) = self.observations.get_mut(step_id) {
                                match port_answer_checksum(&answer) {
                                    Ok(checksum) => observation.answer_checksum = Some(checksum),
                                    Err(error) => observation.observation_error = Some(error),
                                }
                            }
                            dispatched = true;
                            // Retain the actual answer before attempting a resume. Failure
                            // of the resume or result contract can never replay this effect.
                            self.host_answers.push((step_id.to_owned(), answer));
                            let answer =
                                match &self.host_answers.last().expect("retained host answer").1 {
                                    PortAnswer::Return { value } => PortAnswer::Return {
                                        value: value.clone(),
                                    },
                                    PortAnswer::DomainError { reason_kind } => {
                                        PortAnswer::DomainError {
                                            reason_kind: reason_kind.clone(),
                                        }
                                    }
                                    PortAnswer::TerminalError { reason_kind } => {
                                        PortAnswer::TerminalError {
                                            reason_kind: reason_kind.clone(),
                                        }
                                    }
                                };
                            self.exchange(
                                transport,
                                RecipeCommand::ResumeHost {
                                    context,
                                    key,
                                    answer,
                                },
                            )
                            .await?;
                        }
                    }
                }
                event => {
                    self.latest_snapshot
                        .as_mut()
                        .expect("observed snapshot")
                        .recipe = Some(event);
                    return Err(RetainedExecutionError::Invalid(
                        "child failed or returned a foreign boundary",
                    ));
                }
            }
        }
    }
}
