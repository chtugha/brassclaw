//! Executes only the step requested by Monty, using the already retained IBS
//! program and one task-owned child context. It never selects the next step,
//! looks up latest, retries an effect or turns a Recipe failure into No-Match.
//! The catalogue owner must establish approval and retain actual Tool adapters
//! before exposing this primitive to production. Preparation alone is no grant.

use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use brassclaw_monty_host::{
    process::{
        PortAnswer, ProcessError, ProcessSnapshot, RecipeBoundary, RecipeCommand, RecipeContextId,
        RecipeEvent, SelectedPython, TaskHandle, WorkerCommand,
    },
    transport_actor::{ActorFailure, RequestId, SubmitError, TransportClient},
};
use brassclaw_skills::association_contract::FailureAction;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::memory::{
    composition::ComposedProgram,
    retained_inputs::{RetainedInputError, RetainedRecipeInputs, RetainedUnboundProgram},
    retained_tools::{RetainedToolBinding, RetainedToolError, RetainedToolProgram},
};

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
    fn binding(&self, step: &str) -> Option<&RetainedToolBinding> {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Failed,
}

/// Private task state; never serialize its routing handles or Tool evidence
/// into model context. Results handed back to Monty are typed data. A dropped
/// step future leaves this execution fenced in Running; an error leaves Failed.
/// Neither permits another feed or implicit replay. The supervisor retains the
/// transport ledger and releases the task's actual contexts during settlement.
pub struct RetainedRecipeExecution {
    task: TaskHandle,
    program: RetainedProgram,
    context: Option<RecipeContextId>,
    phase: Phase,
    started: BTreeSet<String>,
    latest_snapshot: Option<ProcessSnapshot>,
    host_answers: Vec<(String, PortAnswer)>,
    transport_failure: Option<RetainedTransportEvidence>,
}
impl RetainedRecipeExecution {
    pub fn new(task: TaskHandle, program: RetainedProgram) -> Result<Self, RetainedExecutionError> {
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
            context: None,
            phase: Phase::Idle,
            started: BTreeSet::new(),
            latest_snapshot: None,
            host_answers: Vec::new(),
            transport_failure: None,
        })
    }

    pub fn latest_snapshot(&self) -> Option<&ProcessSnapshot> {
        self.latest_snapshot.as_ref()
    }
    /// Actual completed host answers survive a subsequent validation/IPC error.
    /// They are evidence, never permission to repeat a completed invocation.
    pub fn host_answers(&self) -> &[(String, PortAnswer)] {
        &self.host_answers
    }
    pub fn transport_failure(&self) -> Option<&RetainedTransportEvidence> {
        self.transport_failure.as_ref()
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
        let python = SelectedPython {
            source: step.executable_code.clone(),
            checksum: Sha256::digest(step.executable_code.as_bytes()).into(),
            aliases,
        };
        self.phase = Phase::Running;
        self.started.insert(step_id.to_owned());
        let result = self
            .feed(transport, step_id, inputs, python, binding, tools)
            .await;
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
        binding: Option<&RetainedToolBinding>,
        tools: Option<&dyn RetainedToolPort>,
    ) -> Result<Value, RetainedExecutionError> {
        if self.context.is_none() {
            self.exchange(
                transport,
                RecipeCommand::Open {
                    task: self.task,
                    parent: None,
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
                            self.program
                                .inputs()
                                .validate_step_result(step_id, &value)?;
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
