//! Test-only closed facts from the existing owner's real service join.
//! Never serialize commands, inputs, host results, tracebacks or error text.
use super::*;
use brassclaw_monty_host::process::{ProcessBoundary, ProcessError, WorkerCommand};
use serde_json::{Value, json};

fn process(error: &ProcessError) -> Value {
    let class = error
        .python_exception
        .as_ref()
        .map(|observed| match observed.class() {
            "ValueError" => "ValueError",
            "TypeError" => "TypeError",
            "KeyError" => "KeyError",
            "NameError" => "NameError",
            "RuntimeError" => "RuntimeError",
            "AttributeError" => "AttributeError",
            "AssertionError" => "AssertionError",
            "MemoryError" => "MemoryError",
            "TimeoutError" => "TimeoutError",
            "RecursionError" => "RecursionError",
            _ => "private_exception_class",
        });
    let command = error.command.as_deref().map(|command| match command {
        WorkerCommand::Boot { .. } => "boot",
        WorkerCommand::Admit { .. } => "admit",
        WorkerCommand::Resolve { .. } => "resolve",
        WorkerCommand::Recipe { .. } => "recipe",
        WorkerCommand::Inspect => "inspect",
        _ => "other_control",
    });
    json!({"kind":format!("{:?}",error.kind),"command_kind":command,"python_class":class,
        "reaped":error.exit_status.is_some(),"exit_success":error.exit_status.as_ref().map(|status|status.success()),
        "containment_error":error.containment_error.is_some(),"reap_error":error.reap_error.is_some()})
}

pub(crate) fn joined(
    service: &Result<ServiceExit, ServiceFailure>,
    ownership: Option<OwnershipError>,
) -> Value {
    let ownership = ownership.map(|kind| format!("{kind:?}"));
    let exit = match service {
        Ok(exit) => exit,
        Err(kind) => {
            return json!({"phase":"service_join_returned","service_join_error":format!("{kind:?}"),
            "ownership_failure":ownership});
        }
    };
    let transport = match &exit.transport {
        Err(kind) => json!({"error":format!("{kind:?}")}),
        Ok(exit) => json!({"kind":format!("{:?}",exit.kind),"reaped":exit.exit_status.is_some(),
            "exit_success":exit.exit_status.as_ref().map(|status|status.success()),
            "containment_error":exit.containment_error.is_some(),"reap_error":exit.reap_error.is_some(),
            "shutdown_failure":exit.shutdown_failure.as_ref().map(process)}),
    };
    let boundary = match &exit.last_snapshot.boundary {
        None => "none",
        Some(ProcessBoundary::Stopped) => "stopped",
        Some(ProcessBoundary::ControlYield { .. }) => "control_yield",
        Some(ProcessBoundary::Waiting { .. }) => "waiting",
        Some(ProcessBoundary::HostCall { name, .. }) => match name.as_str() {
            "await_next_task" => "host/await_next_task",
            "enter_task" => "host/enter_task",
            "resolve_intent" => "host/resolve_intent",
            "resolve_internal_recipe" => "host/resolve_internal_recipe",
            "compose_orchestrator" => "host/compose_orchestrator",
            "run_program" => "host/run_program",
            "resolve_component_by_name" => "host/resolve_component_by_name",
            "finish_task" => "host/finish_task",
            _ => "host/private_call",
        },
    };
    let first_failure = exit
        .failed_exchanges
        .iter()
        .find_map(|receipt| receipt.outcome.as_ref().err())
        .map(process);
    json!({"phase":"service_join_returned","service_join_error":Value::Null,
        "ownership_failure":ownership,"service_failure":exit.failure.map(|kind|format!("{kind:?}")),
        "transport":transport,"tasks_retained":exit.tasks.len(),"pending_admission":exit.pending_admission.is_some(),
        "queued_admissions":exit.queued_admissions.len(),"failed_exchanges":exit.failed_exchanges.len(),
        "rejected_commands":exit.rejected_commands.len(),"first_failed_exchange":first_failure,"last_boundary":boundary})
}
