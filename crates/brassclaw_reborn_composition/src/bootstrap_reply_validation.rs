//! Noncircular, installation-owned reply qualification. Only the exact bundled
//! candidate enters here. Effects are contained in isolated durable validation
//! chats, using the actual reply handler, kernel and invocation journal.
use crate::{RebornBuildError, monty_kernel::MontyKernelSnapshot};
use async_trait::async_trait;
use brassclaw_authorization::{
    InstanceToolPolicyError, InstanceToolPolicySource, InstanceToolRule,
};
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedExecutionError, RetainedProgram, RetainedRecipeExecution, RetainedStepFailure,
        RetainedTransportEvidence, port_answer_checksum, typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_host_api::CapabilityDescriptor;
use brassclaw_monty_host::{
    VmBounds, VmFailure,
    global::{GlobalBounds, Lifecycle},
    heap::HeapSettings,
    process::{
        PortAnswer, ProcessFailure, ProcessLimits, RecipeCommand, RootBoot, TaskSettings,
        WorkerCommand,
    },
    transport_actor::ActorLimits,
};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};

pub(super) fn invalid(reason: impl ToString) -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: reason.to_string(),
    }
}
pub(super) const ROOT: &str = "import asyncio\nasync def validate_reply():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    await host.validation_hold(task['task_token'])\nasyncio.run(validate_reply())\n";
pub(super) const PRODUCER: &str = "brassclaw-public-reply-bootstrap/1";

// Fixed packaged acceptance cases. No caller-supplied expected success or source.
pub(super) fn cases() -> Value {
    json!([
        {"answer":"Ready.","denied":false,"outcome":"reply"},
        {"answer":"quoted ' Unicode ü and {{not_source}}","denied":false,"outcome":"reply"},
        {"answer":"first line\nsecond line with \\ and %","denied":false,"outcome":"reply"},
        {"answer":"policy-blocked qualification case","denied":true,"outcome":"retained_tool_authorization"},
        {"answer":" \n\t","denied":false,"outcome":"retained_tool_invalid_input"}
    ])
}

/// Restrict the current global rule; never override a block to enable a fixture.
pub(crate) struct RestrictedPolicy {
    pub(crate) live: Arc<dyn InstanceToolPolicySource>,
    pub(crate) denied: bool,
}
#[async_trait]
impl InstanceToolPolicySource for RestrictedPolicy {
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        let mut rule = self.live.current_rule(descriptor).await?;
        if self.denied
            && let Some(rule) = &mut rule
        {
            rule.enabled = false;
        }
        Ok(rule)
    }
}

pub(super) async fn execute_cases(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    inspected: Arc<InspectedRetainedProgram>,
    worker: &Path,
) -> Result<Value, RebornBuildError> {
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("bootstrap requires Tool binding"));
    };
    for bad in [
        json!({}),
        json!({"answer":null}),
        json!({"answer":3}),
        json!({"answer":"valid","foreign":[]}),
    ] {
        if program.inputs().bind_task_inputs(&bad).is_ok() {
            return Err(invalid("bootstrap invalid task input was accepted"));
        }
    }
    let mut observations = Vec::new();
    for case in cases()
        .as_array()
        .ok_or_else(|| invalid("bootstrap cases missing"))?
    {
        let answer = case["answer"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap answer missing"))?;
        let denied = case["denied"]
            .as_bool()
            .ok_or_else(|| invalid("bootstrap policy missing"))?;
        let admitted = Box::pin(crate::bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-reply",
            answer,
        ))
        .await?;
        let tools = crate::installed_monty_catalogue::bootstrap_reply_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            denied,
        )?;
        let boot = RootBoot {
            adapter_reserve_bytes: brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize,
            source: ROOT.into(),
            checksum: Sha256::digest(ROOT.as_bytes()).into(),
            aliases: ["await_next_task", "enter_task", "validation_hold"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            bounds: GlobalBounds {
                values: VmBounds {
                    max_source_bytes: 32768,
                    max_compiled_source_bytes: 65536,
                    max_feeds: 32,
                    max_stdout_bytes: 1024,
                    execution_slice: Duration::from_millis(5),
                    max_value_depth: 32,
                    max_value_nodes: 4096,
                    max_value_bytes: 65536,
                },
                workers: 1,
                max_pending_calls: 8,
            },
            heap_settings: Some(HeapSettings {
                revision: 1,
                max_vm_bytes: 32 * 1024 * 1024,
            }),
            startup_timeout: Duration::from_secs(10),
            task_settings: TaskSettings {
                revision: 1,
                max_compute_time: Duration::from_secs(10),
                token_budgets_enabled: false,
            },
            max_recipe_contexts: 8,
        };
        let (mut owner, transport, root, task) = crate::bootstrap_validation::start(
            worker,
            boot,
            ProcessLimits {
                hard_memory_bytes: 128 * 1024 * 1024,
                max_frame_bytes: 1024 * 1024,
                response_timeout: Duration::from_secs(10),
            },
            ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * 1024 * 1024,
                max_control_unclaimed: 8,
                max_control_reserved_frame_bytes: 4 * 1024 * 1024,
            },
            admitted.input.clone(),
        )
        .await?;
        let input = program
            .inputs()
            .bind_task_inputs(&json!({"answer":answer}))
            .map_err(invalid)?;
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected.clone())
                .map_err(invalid)?;
        let result = execution
            .run_step(&transport, "0:2", &input, Some(tools.as_ref()))
            .await;
        let actual = execution
            .observations()
            .get("0:2")
            .filter(|value| value.settled() && value.observation_error().is_none())
            .ok_or_else(|| invalid("bootstrap behavior is not settled"))?;
        let (expected_answer, expected_result, expected_failure) = if case["outcome"] == "reply" {
            let value = result.map_err(invalid)?;
            let reply = admitted
                .host
                .finalized_reply_ref()
                .ok_or_else(|| invalid("bootstrap reply not finalized"))?;
            if serde_json::to_value(&reply).map_err(invalid)? != value
                || admitted
                    .host
                    .published_reply_content(&reply)
                    .map_err(invalid)?
                    != answer
                || !execution.is_complete()
                || execution.transport_failure().is_some()
            {
                return Err(invalid("bootstrap literal reply differs"));
            }
            (
                PortAnswer::Return {
                    value: value.clone().into(),
                },
                Some(value),
                None,
            )
        } else {
            if result.is_ok()
                || execution.failed_step_id() != Some("0:2")
                || admitted.host.finalized_reply_ref().is_some()
            {
                return Err(invalid(
                    "bootstrap failure produced a reply or wrong termination",
                ));
            }
            // Terminal Tool answers abort the child feed. Preserve and qualify
            // that exact VM failure; an expected policy/input rejection is not
            // a successful feed, nor permission to ignore a transport fault.
            let Err(RetainedExecutionError::Process(error)) = &result else {
                return Err(invalid(
                    "bootstrap negative case lacks actual child failure",
                ));
            };
            let Some(RetainedTransportEvidence::Process {
                error: retained,
                command,
                ..
            }) = execution.transport_failure()
            else {
                return Err(invalid("bootstrap child failure evidence missing"));
            };
            if !Arc::ptr_eq(error, retained)
                || retained.kind != ProcessFailure::Vm(VmFailure::Python)
                || retained.snapshot.as_ref().is_none_or(|snapshot| {
                    snapshot.lifecycle != Lifecycle::Ready || snapshot.root != root.root
                })
                || !command.as_ref().is_ok_and(|command| {
                    matches!(
                        command.as_ref(),
                        WorkerCommand::Recipe {
                            command: RecipeCommand::ResumeHost { .. }
                        }
                    )
                })
            {
                return Err(invalid(
                    "bootstrap negative case has a different runtime failure",
                ));
            }
            (
                PortAnswer::TerminalError {
                    reason_kind: case["outcome"]
                        .as_str()
                        .ok_or_else(|| invalid("bootstrap outcome missing"))?
                        .into(),
                },
                None,
                Some(RetainedStepFailure::Process),
            )
        };
        let checks = [
            ("single host answer", execution.host_answers().len() == 1),
            (
                "step order",
                execution.observation_order() == Some(["0:2".to_owned()].as_slice()),
            ),
            (
                "typed inputs",
                actual.input_checksum() == typed_value_checksum(&input).map_err(invalid)?,
            ),
            (
                "Tool arguments",
                actual.arguments_checksum()
                    == Some(typed_value_checksum(&json!({"answer":answer})).map_err(invalid)?),
            ),
            (
                "Tool answer",
                actual.answer_checksum()
                    == Some(port_answer_checksum(&expected_answer).map_err(invalid)?),
            ),
            (
                "returned result",
                actual.result_checksum()
                    == expected_result
                        .as_ref()
                        .map(typed_value_checksum)
                        .transpose()
                        .map_err(invalid)?,
            ),
            (
                "failure classification",
                actual.failure() == expected_failure,
            ),
        ];
        if let Some((check, _)) = checks.iter().find(|(_, passed)| !passed) {
            let answer_kind = match execution.host_answers().first() {
                Some((_, PortAnswer::TerminalError { reason_kind })) => reason_kind.as_str(),
                Some((_, PortAnswer::Return { .. })) => "return",
                _ => "other",
            };
            return Err(invalid(format!(
                "bootstrap observed {check} differs for {} case (Tool answer: {answer_kind})",
                case["outcome"],
            )));
        }
        // Retain actual transcript records and no-replay journals before approval.
        let observation = json!({"case":case,"run_id":admitted.attempt.run_id.as_uuid(),
            "thread_id":admitted.host.run_context().thread_id,
            "reply_ref":expected_result,"arguments_checksum":hex(actual.arguments_checksum().ok_or_else(|| invalid("bootstrap arguments missing"))?),
            "answer_checksum":hex(actual.answer_checksum().ok_or_else(|| invalid("bootstrap answer missing"))?),
            "failure":actual.failure().map(RetainedStepFailure::reason_kind),
            "worker_root":root.root});
        crate::bootstrap_validation::close(&mut owner, &admitted).await?;
        observations.push(observation);
    }
    Ok(
        json!({"cases":observations,"invalid_inputs_rejected":true,"root_checksum":hex(Sha256::digest(ROOT.as_bytes()).into())}),
    )
}
pub(super) fn hex(value: [u8; 32]) -> String {
    hex::encode(value)
}
pub(super) fn root_checksum() -> String {
    hex(Sha256::digest(ROOT.as_bytes()).into())
}
