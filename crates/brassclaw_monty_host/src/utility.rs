//! Contained authoring utilities for the 1.0 API cutover.
//!
//! Parsing and pure formatting are separate from the instance orchestrator.
//! Each operation has its own disposable allocator worker and no host ports,
//! catalogue selection, Tool dispatch, approval, task sequencing or retries.
//! The Engine syntax/formatter adapter and Composition Q1 use this boundary.
use std::{
    collections::BTreeMap,
    fmt, io,
    path::Path,
    process::{ExitStatus, Stdio},
    time::Duration,
};

use monty::MontyRun;
use monty_types::{
    CompileOptions, OsPolicy, PrintWriter, ResourceLimits, ResourceTracker, SleepMode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use crate::{
    VmBounds, VmError, VmFailure, identifier, json_input, json_output,
    process::{ProcessFailure, ProcessLimits, encode, read_frame, transport_value, write_frame},
};

const PROTOCOL: u32 = 3;

/// Caller-authored utility source is intentionally not an approved component.
/// Data remains separately injected values, never Python source substitution.
#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum UtilityRequest {
    Parse {
        source: String,
        bounds: VmBounds,
    },
    InspectSource {
        source: String,
        bounds: VmBounds,
    },
    Evaluate {
        source: String,
        inputs: BTreeMap<String, Value>,
        bounds: VmBounds,
        max_compute_time: Duration,
    },
}
impl UtilityRequest {
    fn bounds(&self) -> VmBounds {
        match self {
            Self::Parse { bounds, .. }
            | Self::InspectSource { bounds, .. }
            | Self::Evaluate { bounds, .. } => *bounds,
        }
    }
    fn valid(&self, frame_limit: usize) -> bool {
        let (source, bounds) = match self {
            Self::Parse { source, bounds }
            | Self::InspectSource { source, bounds }
            | Self::Evaluate { source, bounds, .. } => (source, bounds),
        };
        bounds.valid()
            && source.len() <= bounds.max_source_bytes
            && match self {
                Self::Parse { .. } | Self::InspectSource { .. } => true,
                Self::Evaluate {
                    inputs,
                    max_compute_time,
                    ..
                } => {
                    !max_compute_time.is_zero()
                        && std::time::Instant::now().checked_add(*max_compute_time).is_some()
                                                // Borrowed preflight aggregates all input names and values.
                        // No nested data is serialized before this check.
                        && inputs_within_bounds(inputs.iter().map(|(name, value)| (name.as_str(), value)), *bounds, frame_limit)
                }
            }
    }
}

/// Borrowed aggregate preflight for callers before cloning or serializing data.
/// Exceeding a technical bound is an error, never silent truncation.
pub fn inputs_within_bounds<'a>(
    inputs: impl IntoIterator<Item = (&'a str, &'a Value)>,
    bounds: VmBounds,
    frame_limit: usize,
) -> bool {
    if !bounds.valid() {
        return false;
    }
    // Share one aggregate budget across roots without cloning an input tree.
    // transport_value checks each root's depth as well as traversal size; the
    // explicit counters here enforce total nodes/bytes over all roots.
    let mut remaining_nodes = bounds.max_value_nodes;
    let mut remaining_bytes = bounds.max_value_bytes;
    for (name, value) in inputs {
        if !identifier(name) {
            return false;
        }
        let Some(bytes) = remaining_bytes.checked_sub(name.len()) else {
            return false;
        };
        remaining_bytes = bytes;
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            let Some(nodes) = remaining_nodes.checked_sub(1) else {
                return false;
            };
            remaining_nodes = nodes;
            let bytes = match value {
                Value::String(value) => value.len(),
                Value::Object(values) => {
                    // Check before pushing an entire container into traversal.
                    if values
                        .len()
                        .checked_add(pending.len())
                        .is_none_or(|n| n > remaining_nodes)
                    {
                        return false;
                    }
                    let Some(bytes) = values
                        .keys()
                        .try_fold(0usize, |n, key| n.checked_add(key.len()))
                    else {
                        return false;
                    };
                    pending.extend(values.values());
                    bytes
                }
                Value::Array(values) => {
                    if values
                        .len()
                        .checked_add(pending.len())
                        .is_none_or(|n| n > remaining_nodes)
                    {
                        return false;
                    }
                    pending.extend(values);
                    0
                }
                _ => 0,
            };
            let Some(bytes) = remaining_bytes.checked_sub(bytes) else {
                return false;
            };
            remaining_bytes = bytes;
        }
        if !transport_value(value, bounds, frame_limit) {
            return false;
        }
    }
    true
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UtilityOutput {
    Parsed,
    Inspected {
        structure: crate::source_structure::SourceStructure,
    },
    Evaluated {
        value: Value,
        stdout: String,
    },
}
impl fmt::Debug for UtilityOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Parsed => "Parsed",
            Self::Inspected { .. } => "Inspected(<private source structure>)",
            Self::Evaluated { .. } => "Evaluated(<private result>)",
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u32,
    command: UtilityRequest,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    protocol: u32,
    output: Option<UtilityOutput>,
    failure: Option<VmFailure>,
    diagnostic: Option<String>,
    stdout: String,
}

/// Private diagnostics and original data remain available without being logged.
/// Reaping proves utility-worker termination, not component approval or effects.
pub struct UtilityError {
    pub kind: ProcessFailure,
    pub request: Box<UtilityRequest>,
    pub diagnostic: Option<String>,
    pub stdout: String,
    pub exit_status: Option<ExitStatus>,
    pub containment_error: Option<io::Error>,
    pub reap_error: Option<io::Error>,
}
impl fmt::Debug for UtilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UtilityError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for UtilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Monty utility failed: {:?}", self.kind)
    }
}
impl std::error::Error for UtilityError {}

/// One contained operation. A successful result requires both its actual reply
/// and successful OS exit. No parser or evaluator runs in the application.
pub async fn execute(
    executable: &Path,
    request: UtilityRequest,
    limits: ProcessLimits,
) -> Result<UtilityOutput, UtilityError> {
    let mut error = UtilityError {
        kind: ProcessFailure::InvalidLimits,
        request: Box::new(request),
        diagnostic: None,
        stdout: String::new(),
        exit_status: None,
        containment_error: None,
        reap_error: None,
    };
    if !limits.valid() {
        return Err(error);
    }
    if !executable.is_absolute() {
        error.kind = ProcessFailure::InvalidExecutable;
        return Err(error);
    }
    if !error.request.valid(limits.max_frame_bytes) {
        error.kind = ProcessFailure::ValueLimit;
        return Err(error);
    }
    // Encode a borrowed request, retaining the exact caller payload on failure.
    #[derive(Serialize)]
    struct BorrowedRequest<'a> {
        protocol: u32,
        command: &'a UtilityRequest,
    }
    let frame = match encode(
        &BorrowedRequest {
            protocol: PROTOCOL,
            command: &error.request,
        },
        limits.max_frame_bytes,
    ) {
        Ok(frame) => frame,
        Err(_) => {
            error.kind = ProcessFailure::FrameLimit;
            return Err(error);
        }
    };
    let mut child = match Command::new(executable)
        .env_clear()
        .arg(limits.hard_memory_bytes.to_string())
        .arg(limits.max_frame_bytes.to_string())
        .arg("--utility")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            error.kind = ProcessFailure::Spawn;
            return Err(error);
        }
    };
    let operation = async {
        let mut input = child.stdin.take().ok_or(ProcessFailure::Transport)?;
        input
            .write_all(&(frame.len() as u32).to_be_bytes())
            .await
            .map_err(|_| ProcessFailure::Transport)?;
        input
            .write_all(&frame)
            .await
            .map_err(|_| ProcessFailure::Transport)?;
        input
            .shutdown()
            .await
            .map_err(|_| ProcessFailure::Transport)?;
        drop(input);
        let mut output = child.stdout.take().ok_or(ProcessFailure::Transport)?;
        let length = output
            .read_u32()
            .await
            .map_err(|_| ProcessFailure::Transport)? as usize;
        if length == 0 || length > limits.max_frame_bytes {
            return Err(ProcessFailure::Protocol);
        }
        let mut bytes = vec![0; length];
        output
            .read_exact(&mut bytes)
            .await
            .map_err(|_| ProcessFailure::Transport)?;
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| ProcessFailure::Protocol)?;
        if reply.protocol != PROTOCOL || reply.output.is_some() == reply.failure.is_some() {
            return Err(ProcessFailure::Protocol);
        }
        // EOF is part of this one-operation protocol. Extra frames are rejected.
        let mut trailing = [0];
        if output
            .read(&mut trailing)
            .await
            .map_err(|_| ProcessFailure::Transport)?
            != 0
        {
            return Err(ProcessFailure::Protocol);
        }
        let status = child.wait().await.map_err(|_| ProcessFailure::Transport)?;
        Ok((reply, status))
    };
    match tokio::time::timeout(limits.response_timeout, operation).await {
        Ok(Ok((reply, status))) => {
            error.exit_status = Some(status);
            error.diagnostic = reply.diagnostic;
            error.stdout = reply.stdout;
            if !status.success() {
                error.kind = ProcessFailure::Transport;
            } else if let Some(failure) = reply.failure {
                error.kind = ProcessFailure::Vm(failure);
            } else if let Some(output) = reply.output {
                let valid = match (&*error.request, &output) {
                    (UtilityRequest::Parse { .. }, UtilityOutput::Parsed) => true,
                    (
                        UtilityRequest::InspectSource { source, bounds },
                        UtilityOutput::Inspected { structure },
                    ) => structure.valid_for(source, *bounds),
                    (
                        UtilityRequest::Evaluate { .. },
                        UtilityOutput::Evaluated { value, stdout },
                    ) => {
                        stdout.len() <= error.request.bounds().max_stdout_bytes
                            && transport_value(
                                value,
                                error.request.bounds(),
                                limits.max_frame_bytes,
                            )
                    }
                    _ => false,
                };
                if valid {
                    return Ok(output);
                }
                error.kind = ProcessFailure::Protocol;
            }
            Err(error)
        }
        failure => {
            error.kind = match failure {
                Err(_) => ProcessFailure::Deadline,
                Ok(Err(kind)) => kind,
                Ok(Ok(_)) => unreachable!(),
            };
            if let Err(cause) = child.start_kill() {
                error.containment_error = Some(cause)
            }
            match tokio::time::timeout(limits.response_timeout, child.wait()).await {
                Ok(Ok(status)) => error.exit_status = Some(status),
                Ok(Err(cause)) => error.reap_error = Some(cause),
                Err(_) => {
                    error.reap_error = Some(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "utility exit was not acknowledged",
                    ))
                }
            }
            Err(error)
        }
    }
}

pub(crate) fn worker_main(frame_limit: usize) -> Result<(), Box<dyn std::error::Error>> {
    let request: Request = read_frame(&mut io::stdin().lock(), frame_limit)
        .map_err(|_| io::Error::other("invalid utility request"))?;
    if request.protocol != PROTOCOL {
        return Err("invalid utility protocol".into());
    }
    let result = {
        let _allocation_scope = monty_alloc::VmAllocationScope::enter()?;
        evaluate(request.command, frame_limit)
    };
    let (output, failure, diagnostic, stdout) = match result {
        Ok(output) => (Some(output), None, None, String::new()),
        Err(error) => (
            None,
            Some(error.failure),
            error.exception.map(|e| e.to_string()),
            error.stdout,
        ),
    };
    write_frame(
        &mut io::stdout().lock(),
        &Reply {
            protocol: PROTOCOL,
            output,
            failure,
            diagnostic,
            stdout,
        },
        frame_limit,
    )?;
    Ok(())
}

fn evaluate(request: UtilityRequest, frame_limit: usize) -> Result<UtilityOutput, VmError> {
    if !request.valid(frame_limit) {
        return Err(VmError::kind(VmFailure::InvalidInputs));
    }
    let (source, inputs, bounds, compute, inspect) = match request {
        UtilityRequest::Parse { source, bounds } => (source, None, bounds, None, false),
        UtilityRequest::InspectSource { source, bounds } => (source, None, bounds, None, true),
        UtilityRequest::Evaluate {
            source,
            inputs,
            bounds,
            max_compute_time,
        } => (source, Some(inputs), bounds, Some(max_compute_time), false),
    };
    let names = inputs
        .as_ref()
        .map(|inputs| inputs.keys().cloned().collect())
        .unwrap_or_default();
    // Adopt code into the controlled domain. The received String itself belongs
    // to transport; moving it into the compiler would evade VM attribution.
    let mut runner = MontyRun::new(
        source.clone(),
        "utility.py",
        names,
        CompileOptions::default(),
    )
    .map_err(|exception| VmError {
        failure: VmFailure::Python,
        exception: Some(Box::new(exception)),
        stdout: String::new(),
        rejected_answer: None,
    })?
    .with_os_policy(OsPolicy {
        sleep: SleepMode::CallHost,
        ..OsPolicy::default()
    });
    let Some(inputs) = inputs else {
        if inspect {
            return Ok(UtilityOutput::Inspected {
                structure: crate::source_structure::inspect(&source, bounds)?,
            });
        }
        return Ok(UtilityOutput::Parsed);
    };
    let values = inputs
        .values()
        .map(|value| json_input(value, bounds))
        .collect::<Result<Vec<_>, _>>()?;
    let tracker = ResourceTracker::new(
        ResourceLimits::default().max_feed_duration(compute.expect("evaluate duration")),
    );
    let mut stdout = String::new();
    let result = runner.run(
        values,
        tracker,
        PrintWriter::CollectString(&mut stdout, Some(bounds.max_stdout_bytes)),
    );
    let value = match result {
        Ok(value) => match json_output(value.as_ref(), bounds) {
            Ok(value) => value,
            Err(mut error) => {
                error.stdout = stdout;
                return Err(error);
            }
        },
        Err(exception) => {
            return Err(VmError {
                failure: VmFailure::Python,
                exception: Some(Box::new(exception)),
                stdout,
                rejected_answer: None,
            });
        }
    };
    Ok(UtilityOutput::Evaluated { value, stdout })
}
