//! Real root-VM boundaries; no successful provider, Recipe or durable finish
//! operation is simulated. Work-wait/shutdown replies are host control events.
use brassclaw_monty_host::global::{
    GlobalBoundary, GlobalBounds, GlobalVm, Lifecycle, RootExecutionAccounting,
};
use brassclaw_monty_host::{HostAnswer, PythonArtifact, RecipeVm, VmBounds, VmFailure};
use brassclaw_resources::{
    LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision, SharedMontyTaskBudget,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

const SOURCE: &str = include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py");
fn bounds() -> GlobalBounds {
    GlobalBounds {
        values: VmBounds {
            max_source_bytes: 32768,
            max_compiled_source_bytes: 65536,
            max_feeds: 32,
            max_stdout_bytes: 1024,
            execution_slice: Duration::from_secs(1),
            max_value_depth: 16,
            max_value_nodes: 2048,
            max_value_bytes: 16384,
        },
        workers: 2,
        max_pending_calls: 8,
    }
}
fn aliases() -> BTreeSet<String> {
    [
        "await_next_task",
        "enter_task",
        "resolve_intent",
        "resolve_component_by_name",
        "compose_orchestrator",
        "run_program",
        "resolve_reply",
        "finish_task",
    ]
    .map(str::to_owned)
    .into()
}
fn start(source: &str, limits: GlobalBounds) -> (GlobalVm, GlobalBoundary) {
    GlobalVm::start(
        Arc::from(source),
        Sha256::digest(source.as_bytes()).into(),
        aliases(),
        limits,
    )
    .unwrap()
}
fn settle_boot(
    vm: &mut GlobalVm,
    mut boundary: GlobalBoundary,
) -> Vec<brassclaw_monty_host::ContinuationKey> {
    assert_eq!(vm.lifecycle(), Lifecycle::Starting);
    for _ in 0..32 {
        boundary = match boundary {
            GlobalBoundary::HostCall(call) => {
                assert_eq!(call.name, "await_next_task");
                assert!(call.kwargs.is_empty());
                assert_eq!(vm.lifecycle(), Lifecycle::Starting);
                vm.defer(call.continuation).unwrap()
            }
            GlobalBoundary::ControlYield(key) => vm.resume_control(key).unwrap(),
            GlobalBoundary::Waiting(keys) => {
                assert_eq!(vm.lifecycle(), Lifecycle::Ready);
                assert_eq!(keys.len(), 2);
                return keys;
            }
            GlobalBoundary::Stopped => panic!("no early final at boot"),
        }
    }
    panic!("boot handshake did not settle");
}
fn task(token: &str, query: &str) -> Value {
    json!({"task_token": token, "conversation_id": format!("reborn-conv-{token}"),
        "message_id": format!("message-{token}"), "run_id": format!("run-{token}"),
        "turn_id": format!("turn-{token}"), "user_input": query, "history": []})
}
fn call(boundary: GlobalBoundary) -> brassclaw_monty_host::HostRequest {
    let GlobalBoundary::HostCall(call) = boundary else {
        panic!("expected host call, got {boundary:?}")
    };
    call
}

fn context_compute(account: &RootExecutionAccounting, context: u32) -> Duration {
    account
        .contexts
        .get(&Some(context))
        .copied()
        .unwrap_or_default()
        + account
            .preparation_contexts
            .get(&Some(context))
            .copied()
            .unwrap_or_default()
        + account
            .adaptation_contexts
            .get(&Some(context))
            .copied()
            .unwrap_or_default()
}

fn task_budget() -> SharedMontyTaskBudget {
    SharedMontyTaskBudget::new(
        LiveMontyTaskSettings::new(MontyTaskSettingsRevision {
            revision: 1,
            limits: MontyTaskLimits {
                max_compute_time: Duration::from_secs(600),
                token_budgets_enabled: false,
            },
        })
        .unwrap(),
    )
}

#[test]
fn root_compute_follows_actual_coroutines_and_resume_owners_across_a_b_a() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        aliases(),
        bounds(),
        Duration::from_secs(5),
    )
    .unwrap();
    let waits = vm.work_waits();
    let a = vm.pending_context(waits[0].1).unwrap();
    let b = vm.pending_context(waits[1].1).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, 0);
    assert_ne!(b, 0);
    let budget_a = task_budget();
    let budget_b = task_budget();
    let boot = vm.execution_accounting().unwrap();
    let intent_a = call(
        vm.admit(waits[0].1, task("A", "first"), budget_a.clone())
            .unwrap(),
    );
    assert_eq!(intent_a.name, "resolve_intent");
    assert_eq!(vm.pending_context(intent_a.continuation), Some(a));
    assert!(matches!(
        vm.defer(intent_a.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    let after_a = vm.execution_accounting().unwrap();
    assert!(after_a.contexts[&Some(a)] > boot.contexts[&Some(a)]);
    assert_eq!(after_a.contexts[&Some(b)], boot.contexts[&Some(b)]);
    assert_eq!(
        budget_a.check().unwrap().usage.compute_time,
        context_compute(&after_a, a) - context_compute(&boot, a)
    );
    assert_eq!(budget_b.check().unwrap().usage.compute_time, Duration::ZERO);

    let intent_b = call(
        vm.admit(waits[1].1, task("B", "second"), budget_b.clone())
            .unwrap(),
    );
    assert_eq!(intent_b.name, "resolve_intent");
    assert_eq!(vm.pending_context(intent_b.continuation), Some(b));
    assert!(matches!(
        vm.defer(intent_b.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    let after_b = vm.execution_accounting().unwrap();
    assert!(after_b.contexts[&Some(b)] > after_a.contexts[&Some(b)]);
    assert_eq!(after_b.contexts[&Some(a)], after_a.contexts[&Some(a)]);
    // No interpreter run occurs during this real external wait.
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(vm.execution_accounting().unwrap(), after_b);
    assert_eq!(
        budget_a.check().unwrap().usage.compute_time,
        context_compute(&after_a, a) - context_compute(&boot, a)
    );
    assert_eq!(
        budget_b.check().unwrap().usage.compute_time,
        context_compute(&after_b, b) - context_compute(&boot, b)
    );

    // A real host error raises into A's trusted Python handler. No successful
    // Tool, provider, Recipe or durable finish operation is fabricated.
    let finish_a = call(
        vm.resolve(
            intent_a.continuation,
            HostAnswer::Raise(monty_types::MontyException::runtime_error(
                "test host failure",
            )),
        )
        .unwrap(),
    );
    assert_eq!(finish_a.name, "finish_task");
    assert_eq!(finish_a.args[0], json!("A"));
    assert_eq!(vm.pending_context(finish_a.continuation), Some(a));
    let resumed_a = vm.execution_accounting().unwrap();
    assert!(resumed_a.contexts[&Some(a)] > after_b.contexts[&Some(a)]);
    assert_eq!(resumed_a.contexts[&Some(b)], after_b.contexts[&Some(b)]);
    assert!(resumed_a.adaptation > after_b.adaptation);
    assert!(resumed_a.adaptation_contexts[&Some(a)] > after_b.adaptation_contexts[&Some(a)]);
    assert_eq!(
        resumed_a.adaptation_contexts[&Some(b)],
        after_b.adaptation_contexts[&Some(b)]
    );
    assert_eq!(
        resumed_a
            .adaptation_contexts
            .values()
            .copied()
            .sum::<Duration>(),
        resumed_a.adaptation
    );
    assert!(resumed_a.preparation > after_b.preparation);
    assert!(resumed_a.preparation_contexts[&Some(a)] > after_b.preparation_contexts[&Some(a)]);
    assert_eq!(
        resumed_a.preparation_contexts[&Some(b)],
        after_b.preparation_contexts[&Some(b)]
    );
    assert_eq!(
        resumed_a
            .preparation_contexts
            .values()
            .copied()
            .sum::<Duration>(),
        resumed_a.preparation
    );
    assert_eq!(
        resumed_a.contexts.values().copied().sum::<Duration>(),
        resumed_a.execution
    );
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert!(vm.task_active("A").unwrap());
    assert!(vm.task_active("B").unwrap());
    assert_eq!(vm.task_compute_failure("A").unwrap(), None);
    assert_eq!(
        budget_a.check().unwrap().usage.compute_time,
        context_compute(&resumed_a, a) - context_compute(&boot, a)
    );
    // A real child executes against the same account while the root is parked.
    let before_child = budget_a.check().unwrap().usage.compute_time;
    let code = "raise ValueError('actual shared-account child failure')";
    let artifact = Arc::new(
        PythonArtifact::new(
            Arc::from(code),
            Sha256::digest(code.as_bytes()).into(),
            BTreeSet::new(),
            bounds().values,
        )
        .unwrap(),
    );
    let mut child = RecipeVm::new(budget_a.clone(), bounds().values).unwrap();
    assert_eq!(
        child.start_step(artifact, json!({})).unwrap_err().failure,
        VmFailure::Python
    );
    assert!(budget_a.check().unwrap().usage.compute_time > before_child);
    assert_eq!(
        budget_b.check().unwrap().usage.compute_time,
        context_compute(&after_b, b) - context_compute(&boot, b)
    );
    assert_eq!(vm.execution_accounting().unwrap(), resumed_a);
    // Fatal release retains the actual unresolved calls for reconciliation.
    assert_eq!(vm.abandon().len(), 2);
    assert_eq!(vm.execution_accounting().unwrap(), resumed_a);
}

#[test]
fn tracked_admission_rejects_before_binding_and_reused_worker_gets_a_new_account() {
    // Exercise interpreter admission mechanics, not a substituted product
    // workflow or a fake durable finish. This fixture loops after a real error.
    let source = "import asyncio\nasync def worker(i):\n    while True:\n        task = await host.await_next_task(i)\n        if task is None:\n            return\n        try:\n            host.enter_task(task['task_token'])\n            await host.resolve_intent(task['task_token'], task['user_input'])\n        except Exception:\n            pass\n        task = None\nasync def main():\n    await asyncio.gather(worker(0), worker(1))\nasyncio.run(main())";
    let mut vm = GlobalVm::start_ready(
        Arc::from(source),
        Sha256::digest(source.as_bytes()).into(),
        aliases(),
        bounds(),
        Duration::from_secs(5),
    )
    .unwrap();
    let waits = vm.work_waits();
    let context = vm.pending_context(waits[0].1).unwrap();
    let first_budget = task_budget();
    let oversized = task("reused", &"x".repeat(bounds().values.max_value_bytes + 1));
    let rejected = vm
        .admit(waits[0].1, oversized, first_budget.clone())
        .unwrap_err();
    assert_eq!(rejected.failure, VmFailure::InvalidInputs);
    assert!(rejected.rejected_answer.is_some());
    assert!(!vm.task_active("reused").unwrap());
    assert_eq!(
        first_budget.check().unwrap().usage.compute_time,
        Duration::ZERO
    );
    assert_eq!(vm.work_waits(), waits);
    let first = call(
        vm.admit(waits[0].1, task("reused", "first"), first_budget.clone())
            .unwrap(),
    );
    assert_eq!(vm.pending_context(first.continuation), Some(context));
    vm.defer(first.continuation).unwrap();
    let rejected_budget = task_budget();
    let duplicate = vm
        .admit(waits[1].1, task("reused", "other"), rejected_budget.clone())
        .unwrap_err();
    assert_eq!(duplicate.failure, VmFailure::WrongBoundary);
    assert_eq!(
        rejected_budget.check().unwrap().usage.compute_time,
        Duration::ZERO
    );
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    let wait_again = call(
        vm.resolve(
            first.continuation,
            HostAnswer::Raise(monty_types::MontyException::runtime_error(
                "actual task-port error",
            )),
        )
        .unwrap(),
    );
    assert_eq!(wait_again.name, "await_next_task");
    assert_eq!(vm.pending_context(wait_again.continuation), Some(context));
    assert!(!vm.task_active("reused").unwrap());
    vm.defer(wait_again.continuation).unwrap();
    let first_final = first_budget.check().unwrap().usage.compute_time;
    let baseline = context_compute(&vm.execution_accounting().unwrap(), context);
    let next_budget = task_budget();
    let next = call(
        vm.admit(
            wait_again.continuation,
            task("next", "second"),
            next_budget.clone(),
        )
        .unwrap(),
    );
    assert_eq!(vm.pending_context(next.continuation), Some(context));
    assert_eq!(
        first_budget.check().unwrap().usage.compute_time,
        first_final
    );
    assert_eq!(
        next_budget.check().unwrap().usage.compute_time,
        context_compute(&vm.execution_accounting().unwrap(), context) - baseline
    );
    assert!(vm.task_active("next").unwrap());
    assert!(!vm.task_active("reused").unwrap());
    assert_eq!(vm.abandon().len(), 2);
}

#[test]
fn terminal_shared_account_is_visible_without_poisoning_global_vm() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        aliases(),
        bounds(),
        Duration::from_secs(5),
    )
    .unwrap();
    let waits = vm.work_waits();
    let expired = task_budget();
    // Explicit account-state fixture; this is not a claim of measured VM work
    // or successful external execution. Check the terminal-account contract.
    expired
        .record_compute_time(Duration::from_secs(601))
        .unwrap();
    let rejected = vm
        .admit(waits[0].1, task("expired", "input"), expired.clone())
        .unwrap_err();
    assert_eq!(rejected.failure, VmFailure::ResourceLimit);
    assert!(rejected.rejected_answer.is_some());
    assert!(!vm.task_active("expired").unwrap());
    assert_eq!(vm.work_waits(), waits);

    let budget = task_budget();
    let intent = call(
        vm.admit(waits[0].1, task("active", "input"), budget.clone())
            .unwrap(),
    );
    budget
        .record_compute_time(Duration::from_secs(601))
        .unwrap();
    vm.defer(intent.continuation).unwrap();
    assert_eq!(
        vm.task_compute_failure("active").unwrap(),
        Some(brassclaw_resources::MontyTaskBudgetError::ComputeExceeded)
    );
    assert_eq!(
        budget.check().unwrap_err(),
        brassclaw_resources::MontyTaskBudgetError::ComputeExceeded
    );
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_eq!(vm.work_waits(), vec![waits[1]]);
    let finish = call(
        vm.resolve(
            intent.continuation,
            HostAnswer::Raise(monty_types::MontyException::runtime_error(
                "actual task host failure",
            )),
        )
        .unwrap(),
    );
    assert_eq!(finish.name, "finish_task");
    assert_eq!(finish.args[1]["status"], json!("failed"));
    assert_eq!(
        finish.args[1]["reason_kind"],
        json!("task_compute_exceeded")
    );
    let withheld = vm.take_withheld_answers();
    assert_eq!(withheld.len(), 1);
    assert_eq!(withheld[0].continuation, intent.continuation);
    let HostAnswer::Raise(error) = &withheld[0].answer else {
        panic!("the actual host error must remain available for reconciliation")
    };
    assert!(error.to_string().contains("actual task host failure"));
    assert!(vm.take_withheld_answers().is_empty());
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert!(vm.task_active("active").unwrap());
    // Failure reporting does not acknowledge the finish or any external effect.
    assert_eq!(vm.abandon().len(), 2);
}

#[test]
fn root_answer_adapter_exhaustion_retains_the_actual_error_before_python_observes_it() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        aliases(),
        bounds(),
        Duration::from_secs(5),
    )
    .unwrap();
    let waits = vm.work_waits();
    let budget = task_budget();
    let intent = call(
        vm.admit(waits[0].1, task("adapter", "input"), budget.clone())
            .unwrap(),
    );
    vm.defer(intent.continuation).unwrap();
    // Explicit near-limit account fixture; this does not claim 600 seconds
    // of measured work. The adapter interval below is measured, not supplied.
    let consumed = budget.check().unwrap().usage.compute_time;
    budget
        .record_compute_time(Duration::from_secs(600) - consumed)
        .unwrap();
    budget.check().unwrap();
    let before = vm.execution_accounting().unwrap();
    let error = monty_types::MontyException::runtime_error("actual host rejection ".repeat(128));
    let finish = call(
        vm.resolve(intent.continuation, HostAnswer::Raise(error.clone()))
            .unwrap(),
    );
    assert_eq!(finish.name, "finish_task");
    assert_eq!(
        finish.args[1]["reason_kind"],
        json!("task_compute_exceeded")
    );
    assert!(vm.execution_accounting().unwrap().adaptation > before.adaptation);
    let retained = vm.take_withheld_answers();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].continuation, intent.continuation);
    let HostAnswer::Raise(actual) = &retained[0].answer else {
        panic!("original failure must remain a failure");
    };
    assert_eq!(actual.to_string(), error.to_string());
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_eq!(vm.work_waits(), vec![waits[1]]);
    // Actual durable finish remains pending; this is no completion/effect ack.
    assert_eq!(vm.abandon().len(), 2);
}

#[test]
fn task_interruption_stops_root_bytecode_without_ending_other_workers() {
    // Actual interpreter control interruption, with an explicit terminal
    // account fixture. No provider, Tool or durable completion is substituted.
    let source = "import asyncio\nasync def worker(i):\n    task = await host.await_next_task(i)\n    if task is None:\n        return\n    try:\n        host.enter_task(task['task_token'])\n        while True:\n            value = 1 + 1\n    except Exception as error:\n        await host.finish_task(task['task_token'], {'status': 'failed', 'reason_kind': str(error)})\nasync def main():\n    await asyncio.gather(worker(0), worker(1))\nasyncio.run(main())";
    for cancel in [false, true] {
        let mut limits = bounds();
        limits.values.execution_slice = Duration::from_millis(5);
        let mut vm = GlobalVm::start_ready(
            Arc::from(source),
            Sha256::digest(source.as_bytes()).into(),
            aliases(),
            limits,
            Duration::from_secs(5),
        )
        .unwrap();
        let waits = vm.work_waits();
        let budget = task_budget();
        let GlobalBoundary::ControlYield(key) = vm
            .admit(waits[0].1, task("busy", "input"), budget.clone())
            .unwrap()
        else {
            panic!("pure task bytecode must yield to hosting control")
        };
        assert!(budget.check().unwrap().usage.compute_time > Duration::ZERO);
        let reason = if cancel {
            vm.request_task_cancellation("busy").unwrap();
            "task_cancelled"
        } else {
            budget
                .record_compute_time(Duration::from_secs(601))
                .unwrap();
            "task_compute_exceeded"
        };
        let finish = call(vm.resume_control(key).unwrap());
        assert_eq!(finish.name, "finish_task");
        assert_eq!(finish.args[0], json!("busy"));
        assert_eq!(finish.args[1]["reason_kind"], json!(reason));
        assert_eq!(vm.lifecycle(), Lifecycle::Ready);
        assert_eq!(vm.work_waits(), vec![waits[1]]);
        assert!(vm.take_withheld_answers().is_empty());
        assert_eq!(vm.abandon().len(), 2);
    }
}

#[test]
fn actual_global_root_waits_at_boot_and_only_explicit_shutdown_stops_it() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        aliases(),
        bounds(),
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    let inventory = vm.work_waits();
    assert_eq!(
        inventory
            .iter()
            .map(|(worker, _)| *worker)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let waits: Vec<_> = inventory.into_iter().map(|(_, key)| key).collect();
    assert_eq!(vm.outstanding(), waits);
    let error = vm
        .resolve(waits[0], HostAnswer::Return(Value::Null))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::WrongBoundary);
    assert!(error.rejected_answer.is_some());
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_eq!(
        vm.close_worker(waits[0]).unwrap_err().failure,
        VmFailure::WrongBoundary
    );
    vm.begin_shutdown().unwrap();
    assert!(matches!(
        vm.close_worker(waits[0]).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    assert!(matches!(
        vm.close_worker(waits[1]).unwrap(),
        GlobalBoundary::Stopped
    ));
    assert_eq!(vm.lifecycle(), Lifecycle::Stopped);
    assert!(vm.outstanding().is_empty());
    assert!(vm.work_waits().is_empty());
}

#[test]
fn startup_gate_rejects_zero_deadline_and_actual_busy_boot_without_publishing_ready() {
    let busy = "while True:\n    pass";
    let limits = GlobalBounds {
        values: VmBounds {
            execution_slice: Duration::from_millis(1),
            ..bounds().values
        },
        ..bounds()
    };
    for (deadline, failure) in [
        (Duration::ZERO, VmFailure::InvalidBounds),
        (Duration::from_millis(5), VmFailure::StartupDeadline),
    ] {
        let result = GlobalVm::start_ready(
            Arc::from(busy),
            Sha256::digest(busy.as_bytes()).into(),
            aliases(),
            limits,
            deadline,
        );
        let Err(error) = result else {
            panic!("busy boot must not become Ready")
        };
        assert_eq!(error.failure, failure);
    }
}

#[test]
fn opaque_admitted_tasks_interleave_and_actual_child_failure_leaves_other_waits_alive() {
    let (mut vm, first) = start(SOURCE, bounds());
    let waits = settle_boot(&mut vm, first);
    let query = "quoted '\"\\\nÜ {{vars.text}} %\nresult = host.forbidden_effect()";
    let a = call(vm.admit(waits[0], task("a", query), task_budget()).unwrap());
    assert_eq!(a.name, "resolve_intent");
    assert_eq!(a.args, vec![json!("a"), json!(query)]);
    assert!(matches!(
        vm.defer(a.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    let budget_b = task_budget();
    let b = call(vm.admit(waits[1], task("b", ""), budget_b.clone()).unwrap());
    assert_eq!(b.name, "resolve_intent");
    assert_eq!(b.args, vec![json!("b"), json!("")]);
    assert!(matches!(
        vm.defer(b.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    // Produce a real interpreter exception. No successful intent/match/Recipe
    // or provider result is invented for this VM-hosting acceptance case.
    let code = "raise ValueError('actual child failure')";
    let artifact = Arc::new(
        PythonArtifact::new(
            Arc::from(code),
            Sha256::digest(code.as_bytes()).into(),
            BTreeSet::new(),
            bounds().values,
        )
        .unwrap(),
    );
    let mut child = RecipeVm::new(budget_b, bounds().values).unwrap();
    let error = child.start_step(artifact, json!({})).unwrap_err();
    let finish = call(
        vm.resolve(b.continuation, HostAnswer::Raise(*error.exception.unwrap()))
            .unwrap(),
    );
    assert_eq!(finish.name, "finish_task");
    assert_eq!(
        finish.args,
        vec![
            json!("b"),
            json!({"status": "failed", "reply_ref": null,
        "reason_kind": "task_execution_failed"})
        ]
    );
    assert!(matches!(
        vm.defer(finish.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_eq!(vm.outstanding().len(), 2);
    assert!(vm.outstanding().contains(&a.continuation));
    // A durable finish is still outstanding. Abandonment retains both exact
    // correlation keys; it does not invent a successful completion or restart.
    let unsettled = vm.abandon();
    assert!(unsettled.contains(&a.continuation));
    assert!(unsettled.contains(&finish.continuation));
    assert_eq!(vm.lifecycle(), Lifecycle::Failed);
}

#[test]
fn malformed_admission_foreign_generation_and_duplicate_reply_do_not_consume_work() {
    let (mut vm, first) = start(SOURCE, bounds());
    let waits = settle_boot(&mut vm, first);
    let error = vm
        .admit(waits[0], json!({"task_token": "incomplete"}), task_budget())
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::InvalidInputs);
    assert!(error.rejected_answer.is_some());
    assert_eq!(vm.outstanding(), waits);
    let (mut other, first) = start(SOURCE, bounds());
    let other_waits = settle_boot(&mut other, first);
    let error = vm
        .resolve(other_waits[0], HostAnswer::Return(Value::Null))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::ForeignContinuation);
    assert!(error.rejected_answer.is_some());
    vm.begin_shutdown().unwrap();
    vm.close_worker(waits[0]).unwrap();
    let error = vm
        .resolve(waits[0], HostAnswer::Return(Value::Null))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::ForeignContinuation);
    assert!(error.rejected_answer.is_some());
    assert_eq!(vm.outstanding(), vec![waits[1]]);
    other.abandon();
    vm.close_worker(waits[1]).unwrap();
}

#[test]
fn early_final_and_task_ports_at_boot_fail_and_busy_boot_yields_without_ready() {
    for source in ["None", "host.resolve_intent('fake', 'fake')"] {
        assert!(
            GlobalVm::start(
                Arc::from(source),
                Sha256::digest(source.as_bytes()).into(),
                aliases(),
                bounds()
            )
            .is_err()
        );
    }
    let partial =
        "import asyncio\nasync def main():\n    await host.await_next_task(0)\nasyncio.run(main())";
    let (mut partial_vm, boundary) = start(partial, bounds());
    let only_wait = call(boundary);
    let error = partial_vm.defer(only_wait.continuation).unwrap_err();
    assert_eq!(error.failure, VmFailure::WrongBoundary);
    assert_eq!(partial_vm.lifecycle(), Lifecycle::Failed);
    assert_eq!(partial_vm.outstanding(), vec![only_wait.continuation]);
    let limits = GlobalBounds {
        values: VmBounds {
            execution_slice: Duration::from_millis(1),
            ..bounds().values
        },
        ..bounds()
    };
    let (mut vm, boundary) = start("while True:\n    pass", limits);
    assert!(matches!(boundary, GlobalBoundary::ControlYield(_)));
    assert_eq!(vm.lifecycle(), Lifecycle::Starting);
    assert!(vm.abandon().is_empty());
    assert_eq!(vm.lifecycle(), Lifecycle::Failed);
}

#[test]
fn oversized_admission_preserves_ready_vm_and_the_exact_work_wait() {
    let limits = GlobalBounds {
        values: VmBounds {
            max_value_bytes: 128,
            ..bounds().values
        },
        ..bounds()
    };
    let (mut vm, first) = start(SOURCE, limits);
    let waits = settle_boot(&mut vm, first);
    let mut extra = task("a", "");
    extra
        .as_object_mut()
        .unwrap()
        .insert("claim_token".into(), json!("must remain Rust-only"));
    assert_eq!(
        vm.admit(waits[0], extra, task_budget())
            .unwrap_err()
            .failure,
        VmFailure::InvalidInputs
    );
    assert_eq!(vm.outstanding(), waits);
    let error = vm
        .admit(waits[0], task("a", &"ü".repeat(128)), task_budget())
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::InvalidInputs);
    assert!(error.rejected_answer.is_some());
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_eq!(vm.outstanding(), waits);
    vm.begin_shutdown().unwrap();
    vm.close_worker(waits[0]).unwrap();
    assert!(matches!(
        vm.close_worker(waits[1]).unwrap(),
        GlobalBoundary::Stopped
    ));
}
