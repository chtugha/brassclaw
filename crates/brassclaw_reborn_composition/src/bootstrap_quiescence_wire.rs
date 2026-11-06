//! Strict readback of the actual contained receipt/account/OS-exit projection.
//! This checks recorded facts; it neither admits work nor fabricates settlement.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::memory::retained_tools::RetainedToolProgram;
use brassclaw_monty_host::{global::RootExecutionIdentity, process::TaskAccounting};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootReport {
    format: String,
    vm_id: uuid::Uuid,
    source_checksum: String,
    aliases_checksum: String,
    workers: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountReport {
    effective_revision: u64,
    compute_time: Duration,
    failure: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeReport {
    recipe_id: uuid::Uuid,
    selection_checksum: String,
    composed: bool,
    complete: bool,
    completed_steps: Vec<String>,
    pending_step: Value,
    failed_step: Value,
    normal_match: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeCapacity {
    revision: u64,
    max_recipes: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionReport {
    format: String,
    root: RootReport,
    root_completed: bool,
    all_selected_recipes_complete: bool,
    recipes: Vec<RecipeReport>,
    intent_outcome: String,
    reply: Value,
    checked_recipe_capacity: RecipeCapacity,
    accounting: AccountReport,
    withheld_root_answers: usize,
    withheld_host_answers: bool,
    model_exchange_count: usize,
    capability_dispatch_count: usize,
    semantic_approval: bool,
    catalogue_activation: bool,
    internal_receipt: Value,
    controlled_executor_configuration: String,
    controlled_executor_configuration_checksum: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExitStatus {
    code: Value,
    success: bool,
    signal: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerExit {
    root: RootExecutionIdentity,
    status: ExitStatus,
    stop_kind: String,
    retained_tasks: usize,
    pending_admissions: usize,
    queued_admissions: usize,
    failed_exchanges: usize,
    rejected_commands: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Quiescence {
    receipt_root: RootExecutionIdentity,
    accounting: TaskAccounting,
    execution: ExecutionReport,
    worker_exit: WorkerExit,
}
fn fields(value: &Value, expected: &[&str]) -> Result<(), RebornBuildError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("quiescence object required"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("quiescence required fields differ"));
    }
    Ok(())
}
fn duration(value: &Value) -> Result<(), RebornBuildError> {
    fields(value, &["secs", "nanos"])?;
    if value["secs"].as_u64().is_none()
        || value["nanos"].as_u64().is_none_or(|n| n >= 1_000_000_000)
    {
        return Err(invalid("actual compute duration malformed"));
    }
    Ok(())
}
pub(super) fn verify(
    value: &Value,
    program: &RetainedToolProgram,
    task: &Value,
    receipt: &str,
) -> Result<(), RebornBuildError> {
    verify_projection(value, program, task, Some(receipt), None)
}

/// Closed one-step Python-stop fixture only. This never accepts an IPC,
/// resource or service failure and never constructs successful quiescence.
pub(super) fn verify_expected_stop(
    value: &Value,
    program: &RetainedToolProgram,
    task: &Value,
    failed_step: &str,
) -> Result<(), RebornBuildError> {
    if program.program().steplist.len() != 1 {
        return Err(invalid("expected stop requires one fixed fixture step"));
    }
    verify_projection(value, program, task, None, Some(failed_step))
}
fn verify_projection(
    value: &Value,
    program: &RetainedToolProgram,
    task: &Value,
    receipt: Option<&str>,
    failed_step: Option<&str>,
) -> Result<(), RebornBuildError> {
    fields(
        value,
        &["receipt_root", "accounting", "execution", "worker_exit"],
    )?;
    fields(
        &value["accounting"],
        &["task", "effective_revision", "compute_time", "failure"],
    )?;
    duration(&value["accounting"]["compute_time"])?;
    fields(
        &value["worker_exit"],
        &[
            "root",
            "status",
            "stop_kind",
            "retained_tasks",
            "pending_admissions",
            "queued_admissions",
            "failed_exchanges",
            "rejected_commands",
        ],
    )?;
    fields(
        &value["worker_exit"]["status"],
        &["code", "success", "signal"],
    )?;
    let report = &value["execution"];
    fields(
        report,
        &[
            "format",
            "root",
            "root_completed",
            "all_selected_recipes_complete",
            "recipes",
            "intent_outcome",
            "reply",
            "checked_recipe_capacity",
            "accounting",
            "withheld_root_answers",
            "withheld_host_answers",
            "model_exchange_count",
            "capability_dispatch_count",
            "semantic_approval",
            "catalogue_activation",
            "internal_receipt",
            "controlled_executor_configuration",
            "controlled_executor_configuration_checksum",
        ],
    )?;
    fields(
        &report["accounting"],
        &["effective_revision", "compute_time", "failure"],
    )?;
    duration(&report["accounting"]["compute_time"])?;
    fields(
        &report["root"],
        &[
            "format",
            "vm_id",
            "source_checksum",
            "aliases_checksum",
            "workers",
        ],
    )?;
    fields(
        &report["checked_recipe_capacity"],
        &["revision", "max_recipes"],
    )?;
    for recipe in report["recipes"]
        .as_array()
        .ok_or_else(|| invalid("actual recipe accounting missing"))?
    {
        fields(
            recipe,
            &[
                "recipe_id",
                "selection_checksum",
                "composed",
                "complete",
                "completed_steps",
                "pending_step",
                "failed_step",
                "normal_match",
            ],
        )?;
    }
    let wire: Quiescence = serde_json::from_value(value.clone()).map_err(invalid)?;
    let original = crate::global_monty_startup::ControlledExecutorConfiguration::restore_original(
        &wire.execution.controlled_executor_configuration,
    )?;
    let root = wire.receipt_root;
    let actual_task = serde_json::to_value(wire.accounting.task).map_err(invalid)?;
    let task_id: uuid::Uuid = serde_json::from_value(actual_task.clone()).map_err(invalid)?;
    let digest =
        sha2::Sha256::digest(crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED.as_bytes()).into();
    if root.vm_id().is_nil()
        || root.workers() != original.bounds().workers
        || !root.matches_definition(digest, &crate::global_root_seed::ports())
        || root != wire.worker_exit.root
        || actual_task != *task
        || task_id.is_nil()
        || wire.accounting.effective_revision != original.task().revision
        || wire.accounting.failure.is_some()
        || wire.accounting.compute_time.is_none()
        || !wire.worker_exit.status.success
        || wire.worker_exit.status.code != 0
        || !wire.worker_exit.status.signal.is_null()
        || !matches!(
            wire.worker_exit.stop_kind.as_str(),
            "requested" | "graceful"
        )
        || wire.worker_exit.retained_tasks != 0
        || wire.worker_exit.pending_admissions != 0
        || wire.worker_exit.queued_admissions != 0
        || wire.worker_exit.failed_exchanges != 0
        || wire.worker_exit.rejected_commands != 0
    {
        return Err(invalid(
            "actual contained task or worker did not complete successfully",
        ));
    }
    let execution = wire.execution;
    if execution.controlled_executor_configuration_checksum != original.checksum() {
        return Err(invalid("original executor configuration checksum differs"));
    }
    if execution.format != "monty-internal-task-execution/1"
        || execution.root.format != "monty-root-execution/1"
        || execution.root.vm_id != root.vm_id()
        || execution.root.source_checksum != hex::encode(root.source_checksum())
        || execution.root.aliases_checksum != hex::encode(root.aliases_checksum())
        || execution.root.workers != root.workers()
        || execution.root_completed != receipt.is_some()
        || execution.all_selected_recipes_complete != receipt.is_some()
        || execution.intent_outcome != "internal_recipe"
        || !execution.reply.is_null()
        || execution.checked_recipe_capacity.revision != original.capacity().revision
        || usize::try_from(execution.checked_recipe_capacity.max_recipes).map_err(invalid)?
            != original.capacity().max_recipes
        || execution.accounting.effective_revision != wire.accounting.effective_revision
        || Some(execution.accounting.compute_time) != wire.accounting.compute_time
        || !execution.accounting.failure.is_null()
        || execution.withheld_root_answers != 0
        || execution.withheld_host_answers
        || execution.model_exchange_count != 0
        || execution.semantic_approval
        || execution.catalogue_activation
        || execution.internal_receipt != serde_json::json!(receipt)
        || execution.recipes.len() != 1
    {
        return Err(invalid("actual contained receipt/accounting differs"));
    }
    // This count is observability, not a Tool grant or completion substitute.
    let _actual_capability_dispatches = execution.capability_dispatch_count;
    let recipe = &execution.recipes[0];
    let expected = program
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let mut expected_steps = program
        .program()
        .steplist
        .iter()
        .map(|step| step.step_id.as_str())
        .collect::<Vec<_>>();
    if let Some(failed) = failed_step {
        if expected_steps != [failed] || execution.capability_dispatch_count != 0 {
            return Err(invalid("failed fixture does not match original occurrence"));
        }
        expected_steps.clear();
    }
    if recipe.recipe_id != program.inputs().instruction().recipe().uuid
        || recipe.selection_checksum != hex::encode(expected.checksum())
        || !recipe.composed
        || recipe.complete != receipt.is_some()
        || !recipe.pending_step.is_null()
        || recipe.failed_step != serde_json::json!(failed_step)
        || !recipe.normal_match.is_null()
        || recipe
            .completed_steps
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != expected_steps
    {
        return Err(invalid("actual contained Recipe receipt differs"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn assert_rejects_corrupt_readback(
    original: &Value,
    program: &RetainedToolProgram,
    task: &Value,
    receipt: &str,
) {
    verify(original, program, task, receipt).expect("actual successful contained observation");
    // Each mutation starts from the same real successful run, including its
    // original task, selection and OS-exit facts. No replacement receipt is made.
    for path in [
        &["accounting", "failure"][..],
        &["execution", "reply"][..],
        &["execution", "accounting", "failure"][..],
        &["worker_exit", "status", "signal"][..],
        &["execution", "recipes", "0", "pending_step"][..],
        &["execution", "recipes", "0", "failed_step"][..],
        &["execution", "recipes", "0", "normal_match"][..],
    ] {
        let mut changed = original.clone();
        let (key, parents) = path.split_last().expect("nonempty field path");
        let mut parent = &mut changed;
        for part in parents {
            parent = if *part == "0" {
                &mut parent[0]
            } else {
                &mut parent[*part]
            };
        }
        assert!(
            parent[*key].is_null(),
            "nullable field must be explicit in actual observation"
        );
        parent.as_object_mut().expect("field parent").remove(*key);
        assert!(
            verify(&changed, program, task, receipt).is_err(),
            "missing {path:?}"
        );
    }
    for (pointer, replacement) in [
        ("/worker_exit/status/code", serde_json::json!(1)),
        ("/worker_exit/status/success", serde_json::json!(false)),
        ("/worker_exit/status/signal", serde_json::json!(9)),
        (
            "/accounting/compute_time/nanos",
            serde_json::json!(1_000_000_000_u64),
        ),
        (
            "/execution/accounting/compute_time/nanos",
            serde_json::json!(-1),
        ),
        (
            "/execution/recipes/0/completed_steps",
            serde_json::json!([]),
        ),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).expect("actual field present") = replacement;
        assert!(
            verify(&changed, program, task, receipt).is_err(),
            "corrupt {pointer}"
        );
    }
}
use sha2::Digest;
