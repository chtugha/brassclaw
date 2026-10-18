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
fn source_aliases(source: &str) -> BTreeSet<String> {
    let mut ports = aliases();
    if source == SOURCE {
        ports.insert("await_pool_update".into());
    }
    ports
}
fn assert_work_waits(vm: &GlobalVm, waits: &[brassclaw_monty_host::ContinuationKey]) {
    assert_eq!(
        vm.work_waits()
            .iter()
            .map(|(_, key)| *key)
            .collect::<Vec<_>>(),
        waits
    );
    let coordinator = usize::from(vm.worker_pool_capacity().is_ok());
    assert_eq!(vm.outstanding().len(), waits.len() + coordinator);
}
fn abandon_count(vm: &mut GlobalVm) {
    let expected = 2 + usize::from(vm.worker_pool_capacity().is_ok());
    assert_eq!(vm.abandon().len(), expected);
}
fn start(source: &str, limits: GlobalBounds) -> (GlobalVm, GlobalBoundary) {
    GlobalVm::start(
        Arc::from(source),
        Sha256::digest(source.as_bytes()).into(),
        source_aliases(source),
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
                assert!(matches!(
                    call.name.as_str(),
                    "await_next_task" | "await_pool_update"
                ));
                assert!(call.kwargs.is_empty());
                assert_eq!(vm.lifecycle(), Lifecycle::Starting);
                vm.defer(call.continuation).unwrap()
            }
            GlobalBoundary::ControlYield(key) => vm.resume_control(key).unwrap(),
            GlobalBoundary::Waiting(keys) => {
                assert_eq!(vm.lifecycle(), Lifecycle::Ready);
                assert_eq!(
                    keys.len(),
                    2 + usize::from(vm.worker_pool_capacity().is_ok())
                );
                return vm.work_waits().into_iter().map(|(_, key)| key).collect();
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
}

fn assert_owned_observations(
    budget: &SharedMontyTaskBudget,
    after: &RootExecutionAccounting,
    before: &RootExecutionAccounting,
    context: u32,
) {
    let usage = budget.check().unwrap().usage;
    let difference =
        |after: &std::collections::BTreeMap<Option<u32>, Duration>,
         before: &std::collections::BTreeMap<Option<u32>, Duration>| {
            after.get(&Some(context)).copied().unwrap_or_default()
                - before.get(&Some(context)).copied().unwrap_or_default()
        };
    assert_eq!(
        usage.compute_time,
        difference(&after.contexts, &before.contexts)
    );
    assert_eq!(
        usage.preparation_time,
        difference(&after.preparation_contexts, &before.preparation_contexts)
    );
    assert_eq!(
        usage.adaptation_time,
        difference(&after.adaptation_contexts, &before.adaptation_contexts)
    );
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
fn completed_coroutine_accounting_is_retired_before_repeated_worker_replacement() {
    // Pure scheduler preparation followed by actual work waits. No Recipe,
    // provider, task finish or durable effect result is substituted here.
    let source = r#"
import asyncio
async def transient():
    for n in range(8):
        pass
    return None
async def parked(worker_id):
    task = await host.await_next_task(worker_id)
    if task is not None:
        raise RuntimeError('this fixture accepts only hosting shutdown')
async def coordinator():
    await asyncio.gather(transient())
    for iteration in range(2000):
        asyncio._replace_sibling(1, transient())
        await asyncio.gather(transient())
    asyncio._replace_sibling(1, parked(1))
    await parked(0)
async def main():
    await asyncio.gather(coordinator(), transient())
asyncio.run(main())
"#;
    let (mut vm, first) = start(source, bounds());
    let waits = settle_boot(&mut vm, first);
    let account = vm.execution_accounting().unwrap();
    assert!(account.retired_execution > Duration::ZERO);
    assert!(account.contexts.len() <= bounds().workers as usize + 2);
    assert!(account.preparation_contexts.len() <= bounds().workers as usize + 2);
    assert_eq!(
        account.retired_execution + account.contexts.values().copied().sum::<Duration>(),
        account.execution
    );
    assert_eq!(
        account.retired_preparation
            + account
                .preparation_contexts
                .values()
                .copied()
                .sum::<Duration>(),
        account.preparation
    );
    assert_eq!(
        account.retired_adaptation
            + account
                .adaptation_contexts
                .values()
                .copied()
                .sum::<Duration>(),
        account.adaptation
    );
    let identity = vm.execution_identity();
    vm.begin_shutdown().unwrap();
    for wait in waits {
        vm.close_worker(wait).unwrap();
    }
    assert_eq!(vm.lifecycle(), Lifecycle::Stopped);
    assert_eq!(vm.execution_identity(), identity);
    let final_account = vm.execution_accounting().unwrap();
    assert!(final_account.retired_execution >= account.retired_execution);
    assert!(
        final_account
            .contexts
            .keys()
            .all(|key| key.is_none() || *key == Some(0))
    );
    assert_eq!(
        final_account.retired_execution
            + final_account.contexts.values().copied().sum::<Duration>(),
        final_account.execution
    );
}

#[test]
fn unexpected_worker_exit_retains_its_unfinished_task_budget_and_fails_closed() {
    let source = r#"
import asyncio
async def worker(worker_id):
    task = await host.await_next_task(worker_id)
    if task is None:
        return
    host.enter_task(task['task_token'])
    try:
        await host.resolve_intent(task['task_token'], task['user_input'])
    except Exception:
        pass
    # There is deliberately no task-release/work-wait handshake.
await asyncio.gather(worker(0), worker(1))
"#;
    let (mut vm, first) = start(source, bounds());
    let waits = settle_boot(&mut vm, first);
    let budget = task_budget();
    let pending = call(
        vm.admit(waits[0], task("unfinished", "query"), budget.clone())
            .unwrap(),
    );
    let context = vm.pending_context(pending.continuation).unwrap();
    vm.defer(pending.continuation).unwrap();
    let error = vm
        .resolve(
            pending.continuation,
            HostAnswer::Raise(monty_types::MontyException::runtime_error(
                "actual task-port error",
            )),
        )
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::Python);
    assert_eq!(vm.lifecycle(), Lifecycle::Failed);
    assert!(vm.task_active("unfinished").unwrap());
    assert!(
        vm.execution_accounting()
            .unwrap()
            .contexts
            .contains_key(&Some(context))
    );
    assert!(budget.check().unwrap().usage.compute_time > Duration::ZERO);
}

#[test]
fn root_compute_follows_actual_coroutines_and_resume_owners_across_a_b_a() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        source_aliases(SOURCE),
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
    assert_owned_observations(&budget_a, &after_a, &boot, a);
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
    assert_owned_observations(&budget_a, &after_b, &boot, a);
    assert_owned_observations(&budget_b, &after_b, &boot, b);
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
    assert_owned_observations(&budget_a, &resumed_a, &boot, a);
    assert_owned_observations(&budget_b, &resumed_a, &boot, b);
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
    abandon_count(&mut vm);
    assert_eq!(vm.execution_accounting().unwrap(), resumed_a);
}

#[test]
fn tracked_admission_rejects_before_binding_and_unfinished_handoff_retains_ownership() {
    // Exercise interpreter admission mechanics, not a substituted product
    // workflow or a fake durable finish. This deliberately invalid fixture
    // attempts to recycle its worker after an error without finishing the task.
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
    let error = vm
        .resolve(
            first.continuation,
            HostAnswer::Raise(monty_types::MontyException::runtime_error(
                "actual task-port error",
            )),
        )
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::WrongBoundary);
    assert!(error.rejected_answer.is_some());
    assert_eq!(vm.lifecycle(), Lifecycle::Failed);
    assert!(vm.task_active("reused").unwrap());
    assert!(!vm.task_active("next").unwrap());
    assert!(
        vm.execution_accounting()
            .unwrap()
            .contexts
            .contains_key(&Some(context))
    );
    assert!(first_budget.check().unwrap().usage.compute_time > Duration::ZERO);
}

#[test]
fn failed_or_invalid_finish_answers_never_acknowledge_task_handoff() {
    let source = r#"
import asyncio
async def worker(worker_id):
    while True:
        task = await host.await_next_task(worker_id)
        if task is None:
            return
        host.enter_task(task['task_token'])
        try:
            await host.finish_task(task['task_token'], {
                'status': 'failed', 'reply_ref': None,
                'reason_kind': 'task_execution_failed'
            })
        except Exception:
            pass
        # Deliberately attempts handoff even when the completion owner refused.
async def main():
    await asyncio.gather(worker(0), worker(1))
asyncio.run(main())
"#;
    for (answer, expected_failure) in [
        (
            HostAnswer::Raise(monty_types::MontyException::runtime_error(
                "completion refused",
            )),
            VmFailure::WrongBoundary,
        ),
        (
            HostAnswer::Return(json!({"completed": true}).into()),
            VmFailure::InvalidHostArguments,
        ),
    ] {
        let (mut vm, boundary) = start(source, bounds());
        let waits = settle_boot(&mut vm, boundary);
        let budget = task_budget();
        let finish = call(
            vm.admit(waits[0], task("unfinished", "input"), budget.clone())
                .unwrap(),
        );
        assert_eq!(finish.name, "finish_task");
        let context = vm.pending_context(finish.continuation).unwrap();
        vm.defer(finish.continuation).unwrap();
        let error = vm.resolve(finish.continuation, answer).unwrap_err();
        assert_eq!(error.failure, expected_failure);
        assert!(error.rejected_answer.is_some());
        assert_eq!(vm.lifecycle(), Lifecycle::Failed);
        assert!(vm.task_active("unfinished").unwrap());
        assert!(
            vm.execution_accounting()
                .unwrap()
                .contexts
                .contains_key(&Some(context))
        );
        assert!(budget.check().unwrap().usage.compute_time > Duration::ZERO);
    }
}

const POOL_ROOT: &str = include_str!("support/worker_pool.py");

fn start_pool() -> (GlobalVm, GlobalBoundary) {
    let mut ports = aliases();
    ports.insert("await_pool_update".into());
    GlobalVm::start(
        Arc::from(POOL_ROOT),
        Sha256::digest(POOL_ROOT.as_bytes()).into(),
        ports,
        bounds(),
    )
    .unwrap()
}

#[test]
fn pool_startup_counts_its_control_wait_and_rejects_a_second_coordinator() {
    let mut ports = aliases();
    ports.insert("await_pool_update".into());
    let mut too_small = bounds();
    too_small.max_pending_calls = 2;
    assert!(matches!(
        GlobalVm::start(Arc::from(POOL_ROOT), Sha256::digest(POOL_ROOT.as_bytes()).into(), ports.clone(), too_small),
        Err(error) if error.failure == VmFailure::InvalidBounds
    ));
    let mut ready = GlobalVm::start_ready(
        Arc::from(POOL_ROOT),
        Sha256::digest(POOL_ROOT.as_bytes()).into(),
        ports.clone(),
        bounds(),
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(ready.worker_pool_capacity().unwrap().settled());
    ready.abandon();
    let source = "import asyncio\nasync def coordinator():\n    await host.await_pool_update()\nawait asyncio.gather(coordinator(), coordinator())";
    let (mut vm, first) = GlobalVm::start(
        Arc::from(source),
        Sha256::digest(source.as_bytes()).into(),
        ports,
        bounds(),
    )
    .unwrap();
    let first = call(first);
    assert_eq!(first.name, "await_pool_update");
    let error = vm.defer(first.continuation).unwrap_err();
    assert_eq!(error.failure, VmFailure::InvalidHostArguments);
    assert_eq!(vm.lifecycle(), Lifecycle::Failed);
    assert!(vm.outstanding().contains(&first.continuation));
}

fn settle_pool(vm: &mut GlobalVm, mut boundary: GlobalBoundary) {
    for _ in 0..128 {
        boundary = match boundary {
            GlobalBoundary::HostCall(call) => {
                assert!(matches!(
                    call.name.as_str(),
                    "await_next_task" | "await_pool_update"
                ));
                vm.defer(call.continuation).unwrap()
            }
            GlobalBoundary::ControlYield(key) => vm.resume_control(key).unwrap(),
            GlobalBoundary::Waiting(_) => return,
            GlobalBoundary::Stopped => panic!("capacity changes must not stop the root"),
        };
    }
    panic!("pool hosting boundaries did not settle")
}

#[test]
fn actual_pool_grows_drains_and_reuses_slots_without_replacing_global_vm() {
    let (mut vm, first) = start_pool();
    settle_pool(&mut vm, first);
    let identity = vm.execution_identity();
    let original_waits = vm.work_waits();
    assert_eq!(vm.worker_pool_capacity().unwrap().active_ids, vec![0, 1]);
    assert_eq!(vm.pending_call_capacity().minimum, 3);
    let before = vm.worker_pool_capacity().unwrap();
    assert_eq!(
        vm.request_worker_pool(8).unwrap_err().failure,
        VmFailure::InvalidBounds
    );
    assert_eq!(vm.worker_pool_capacity().unwrap(), before);
    for _ in 0..50 {
        let boundary = vm.request_worker_pool(5).unwrap();
        settle_pool(&mut vm, boundary);
        assert_eq!(
            vm.worker_pool_capacity().unwrap().active_ids,
            vec![0, 1, 2, 3, 4]
        );
        let boundary = vm.request_worker_pool(1).unwrap();
        settle_pool(&mut vm, boundary);
        let pool = vm.worker_pool_capacity().unwrap();
        assert_eq!((pool.desired, pool.effective), (1, 5));
        assert!(!pool.settled());
        for (worker_id, wait) in vm.work_waits() {
            if worker_id == 0 {
                assert_eq!(
                    vm.drain_worker(wait).unwrap_err().failure,
                    VmFailure::WrongBoundary
                );
            } else {
                let boundary = vm.drain_worker(wait).unwrap();
                settle_pool(&mut vm, boundary);
            }
        }
        assert_eq!(vm.worker_pool_capacity().unwrap().active_ids, vec![0]);
        assert!(vm.worker_pool_capacity().unwrap().settled());
        assert_eq!(vm.pending_call_capacity().minimum, 2);
        assert_eq!(vm.execution_identity(), identity);
        assert_eq!(vm.work_waits()[0], original_waits[0]);
    }
    vm.begin_shutdown().unwrap();
    vm.close_worker(vm.work_waits()[0].1).unwrap();
    assert!(matches!(
        vm.close_pool_coordinator().unwrap(),
        GlobalBoundary::Stopped
    ));
    assert_eq!(vm.lifecycle(), Lifecycle::Stopped);
    assert_eq!(vm.execution_identity(), identity);
}

#[test]
fn pool_reduction_preserves_busy_worker_future_and_rejects_new_admission_debt() {
    let (mut vm, first) = start_pool();
    settle_pool(&mut vm, first);
    let boundary = vm.request_worker_pool(4).unwrap();
    settle_pool(&mut vm, boundary);
    let busy_wait = vm
        .work_waits()
        .into_iter()
        .find(|(id, _)| *id == 3)
        .unwrap()
        .1;
    let budget = task_budget();
    let busy = call(
        vm.admit(busy_wait, task("busy", "input"), budget.clone())
            .unwrap(),
    );
    vm.defer(busy.continuation).unwrap();
    let consumed = budget.check().unwrap().usage.compute_time;
    let identity = vm.execution_identity();
    let boundary = vm.request_worker_pool(1).unwrap();
    settle_pool(&mut vm, boundary);
    for (id, wait) in vm.work_waits() {
        if id > 0 {
            let rejected = vm
                .admit(wait, task("blocked", "input"), task_budget())
                .unwrap_err();
            assert_eq!(rejected.failure, VmFailure::WrongBoundary);
            let boundary = vm.drain_worker(wait).unwrap();
            settle_pool(&mut vm, boundary);
        }
    }
    assert_eq!(vm.worker_pool_capacity().unwrap().active_ids, vec![0, 3]);
    let fresh = vm.work_waits()[0].1;
    assert_eq!(
        vm.admit(fresh, task("blocked", "input"), task_budget())
            .unwrap_err()
            .failure,
        VmFailure::WrongBoundary
    );
    assert!(vm.outstanding().contains(&busy.continuation));
    assert_eq!(budget.check().unwrap().usage.compute_time, consumed);
    let boundary = vm.request_worker_pool(4).unwrap();
    settle_pool(&mut vm, boundary);
    assert_eq!(
        vm.worker_pool_capacity().unwrap().active_ids,
        vec![0, 1, 2, 3]
    );
    assert!(vm.outstanding().contains(&busy.continuation));
    assert_eq!(vm.execution_identity(), identity);
    // Actual host work is deliberately unresolved; preserve it for supervision.
    assert!(vm.abandon().contains(&busy.continuation));
    assert!(vm.task_active("busy").unwrap());
}

#[test]
fn terminal_shared_account_is_visible_without_poisoning_global_vm() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        source_aliases(SOURCE),
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
    abandon_count(&mut vm);
}

#[test]
fn terminal_account_retains_the_actual_host_error_before_python_observes_it() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        source_aliases(SOURCE),
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
    // Explicit exceeded account fixture, not a claim of measured VM work.
    // Adaptation remains separately measured and cannot cause ComputeExceeded.
    let consumed = budget.check().unwrap().usage.compute_time;
    budget
        .record_compute_time(Duration::from_secs(601) - consumed)
        .unwrap();
    assert_eq!(
        budget.check(),
        Err(brassclaw_resources::MontyTaskBudgetError::ComputeExceeded)
    );
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
    abandon_count(&mut vm);
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
        abandon_count(&mut vm);
    }
}

#[test]
fn actual_global_root_waits_at_boot_and_only_explicit_shutdown_stops_it() {
    let mut vm = GlobalVm::start_ready(
        Arc::from(SOURCE),
        Sha256::digest(SOURCE.as_bytes()).into(),
        source_aliases(SOURCE),
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
    assert_work_waits(&vm, &waits);
    let error = vm
        .resolve(waits[0], HostAnswer::Return(Value::Null.into()))
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
        GlobalBoundary::Waiting(_)
    ));
    assert!(matches!(
        vm.close_pool_coordinator().unwrap(),
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
        "reason_kind": "task_execution_failed", "receipt_ref": null})
        ]
    );
    assert!(matches!(
        vm.defer(finish.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_eq!(vm.outstanding().len(), 3);
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
    assert_work_waits(&vm, &waits);
    let (mut other, first) = start(SOURCE, bounds());
    let other_waits = settle_boot(&mut other, first);
    let error = vm
        .resolve(other_waits[0], HostAnswer::Return(Value::Null.into()))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::ForeignContinuation);
    assert!(error.rejected_answer.is_some());
    vm.begin_shutdown().unwrap();
    vm.close_worker(waits[0]).unwrap();
    let error = vm
        .resolve(waits[0], HostAnswer::Return(Value::Null.into()))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::ForeignContinuation);
    assert!(error.rejected_answer.is_some());
    assert_work_waits(&vm, &[waits[1]]);
    other.abandon();
    vm.close_worker(waits[1]).unwrap();
    assert!(matches!(
        vm.close_pool_coordinator().unwrap(),
        GlobalBoundary::Stopped
    ));
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
    assert_work_waits(&vm, &waits);
    let error = vm
        .admit(waits[0], task("a", &"ü".repeat(128)), task_budget())
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::InvalidInputs);
    assert!(error.rejected_answer.is_some());
    assert_eq!(vm.lifecycle(), Lifecycle::Ready);
    assert_work_waits(&vm, &waits);
    vm.begin_shutdown().unwrap();
    vm.close_worker(waits[0]).unwrap();
    assert!(matches!(
        vm.close_worker(waits[1]).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    assert!(matches!(
        vm.close_pool_coordinator().unwrap(),
        GlobalBoundary::Stopped
    ));
}

fn start_dependent_root(limits: GlobalBounds) -> GlobalVm {
    let source = POOL_ROOT.to_owned();
    let mut ports = aliases();
    ports.insert("await_pool_update".into());
    GlobalVm::start_ready(
        Arc::from(source.as_str()),
        Sha256::digest(source.as_bytes()).into(),
        ports,
        limits,
        Duration::from_secs(5),
    )
    .unwrap()
}

#[test]
fn real_dependent_coroutines_preserve_parent_futures_and_fence_exact_child_runs() {
    let mut vm = start_dependent_root(GlobalBounds {
        workers: 1,
        max_pending_calls: 4,
        ..bounds()
    });
    let identity = vm.execution_identity();
    let parent = call(
        vm.admit(
            vm.work_waits()[0].1,
            task("dependency-parent", "actual pending intent"),
            task_budget(),
        )
        .unwrap(),
    );
    let boundary = vm.defer(parent.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    let child_run = uuid::Uuid::new_v4();
    let (worker, boundary) = vm
        .reserve_dependent_worker(parent.continuation, child_run)
        .unwrap();
    assert_eq!(worker, 1);
    settle_pool(&mut vm, boundary);
    assert_eq!(vm.worker_pool_capacity().unwrap().desired, 1);
    assert_eq!(vm.worker_pool_capacity().unwrap().active_ids, [0, 1]);
    assert_eq!(vm.pending_call_capacity().minimum, 3);
    assert!(vm.outstanding().contains(&parent.continuation));
    let waiting = vm.dependent_worker_wait(worker).unwrap();
    let before = vm.outstanding();
    let (_, boundary) = vm
        .reserve_dependent_worker(parent.continuation, child_run)
        .unwrap();
    assert!(matches!(boundary, GlobalBoundary::Waiting(_)));
    assert_eq!(vm.outstanding(), before);
    assert_eq!(
        vm.reserve_dependent_worker(parent.continuation, uuid::Uuid::new_v4())
            .unwrap_err()
            .failure,
        VmFailure::WrongBoundary
    );
    let mut wrong = task("wrong-dependent", "unrelated input");
    wrong["run_id"] = json!(uuid::Uuid::new_v4().to_string());
    let error = vm.admit(waiting, wrong.clone(), task_budget()).unwrap_err();
    assert_eq!(error.failure, VmFailure::WrongBoundary);
    assert!(
        matches!(error.rejected_answer.as_deref(), Some(HostAnswer::Return(value)) if value == wrong)
    );
    assert_eq!(vm.outstanding(), before);
    assert!(!vm.task_active("wrong-dependent").unwrap());
    assert_eq!(
        vm.drain_worker(waiting).unwrap_err().failure,
        VmFailure::WrongBoundary
    );
    let mut input = task("exact-dependent", "child pending intent");
    input["run_id"] = json!(child_run.to_string());
    let mut oversized = input.clone();
    oversized["task_token"] = json!("oversized-dependent");
    oversized["user_input"] = json!("x".repeat(bounds().values.max_value_bytes * 2));
    assert!(vm.admit(waiting, oversized, task_budget()).is_err());
    assert!(!vm.task_active("oversized-dependent").unwrap());
    assert_eq!(vm.dependent_worker_wait(worker), Some(waiting));
    let child = call(vm.admit(waiting, input, task_budget()).unwrap());
    let boundary = vm.defer(child.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    assert_eq!(
        vm.release_dependent_worker(worker, None)
            .unwrap_err()
            .failure,
        VmFailure::WrongBoundary
    );
    let grand_run = uuid::Uuid::new_v4();
    let (grand, boundary) = vm
        .reserve_dependent_worker(child.continuation, grand_run)
        .unwrap();
    assert_eq!(grand, 2);
    settle_pool(&mut vm, boundary);
    let mut input = task("exact-grandchild", "grandchild pending intent");
    input["run_id"] = json!(grand_run.to_string());
    let grandchild = call(
        vm.admit(
            vm.dependent_worker_wait(grand).unwrap(),
            input,
            task_budget(),
        )
        .unwrap(),
    );
    let boundary = vm.defer(grandchild.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    assert_eq!(vm.pending_call_capacity().minimum, 4);
    let before = vm.outstanding();
    assert_eq!(
        vm.reserve_dependent_worker(grandchild.continuation, uuid::Uuid::new_v4())
            .unwrap_err()
            .failure,
        VmFailure::InvalidBounds
    );
    assert_eq!(vm.outstanding(), before);
    assert_eq!(vm.execution_identity(), identity);
    // No Recipe/model/Tool success is supplied. The actual three host futures
    // and their unfinished scopes are retained for trusted containment.
    let retained = vm.abandon();
    for pending in [
        parent.continuation,
        child.continuation,
        grandchild.continuation,
    ] {
        assert!(retained.contains(&pending));
    }
    for token in ["dependency-parent", "exact-dependent", "exact-grandchild"] {
        assert!(vm.task_active(token).unwrap());
    }
}

#[test]
fn dependent_capacity_rejection_and_parent_cancellation_preserve_real_waits() {
    for limit in [2, 3] {
        let mut vm = start_dependent_root(GlobalBounds {
            workers: 1,
            max_pending_calls: limit,
            ..bounds()
        });
        let identity = vm.execution_identity();
        let parent = call(
            vm.admit(
                vm.work_waits()[0].1,
                task("cancel-parent", "pending actual intent"),
                task_budget(),
            )
            .unwrap(),
        );
        let boundary = vm.defer(parent.continuation).unwrap();
        settle_pool(&mut vm, boundary);
        let before = vm.outstanding();
        let child_run = uuid::Uuid::new_v4();
        if limit == 2 {
            assert_eq!(
                vm.reserve_dependent_worker(parent.continuation, child_run)
                    .unwrap_err()
                    .failure,
                VmFailure::InvalidBounds
            );
            assert_eq!(vm.outstanding(), before);
            assert_eq!(vm.pending_call_capacity().minimum, 2);
            assert!(vm.dependent_worker_wait(1).is_none());
        } else {
            let (worker, boundary) = vm
                .reserve_dependent_worker(parent.continuation, child_run)
                .unwrap();
            settle_pool(&mut vm, boundary);
            let wait = vm.dependent_worker_wait(worker).unwrap();
            vm.request_task_cancellation("cancel-parent").unwrap();
            let mut input = task("cancelled-parent-child", "must never enter");
            input["run_id"] = json!(child_run.to_string());
            assert_eq!(
                vm.admit(wait, input, task_budget()).unwrap_err().failure,
                VmFailure::WrongBoundary
            );
            assert!(!vm.task_active("cancelled-parent-child").unwrap());
            assert_eq!(vm.dependent_worker_wait(worker), Some(wait));
            vm.release_dependent_worker(worker, None).unwrap();
            let boundary = vm.drain_worker(wait).unwrap();
            settle_pool(&mut vm, boundary);
            assert_eq!(vm.worker_pool_capacity().unwrap().active_ids, [0]);
            assert!(vm.worker_pool_capacity().unwrap().settled());
        }
        assert_eq!(vm.execution_identity(), identity);
        assert!(vm.abandon().contains(&parent.continuation));
    }
}

#[test]
fn dependent_run_identity_cannot_be_consumed_by_an_unrelated_idle_slot() {
    let mut vm = start_dependent_root(GlobalBounds {
        max_pending_calls: 4,
        ..bounds()
    });
    let ordinary = vm.work_waits()[1].1;
    let parent = call(
        vm.admit(
            vm.work_waits()[0].1,
            task("reserved-parent", "pending"),
            task_budget(),
        )
        .unwrap(),
    );
    let boundary = vm.defer(parent.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    let child_run = uuid::Uuid::new_v4();
    let (worker, boundary) = vm
        .reserve_dependent_worker(parent.continuation, child_run)
        .unwrap();
    settle_pool(&mut vm, boundary);
    assert_eq!(worker, 2);
    let before = vm.outstanding();
    let mut input = task("stolen-child", "must not enter ordinary slot");
    input["run_id"] = json!(child_run.to_string());
    assert_eq!(
        vm.admit(ordinary, input, task_budget())
            .unwrap_err()
            .failure,
        VmFailure::WrongBoundary
    );
    assert!(!vm.task_active("stolen-child").unwrap());
    assert_eq!(vm.outstanding(), before);
    let other_run = uuid::Uuid::new_v4();
    let mut input = task("other-parent", "another real pending intent");
    input["run_id"] = json!(other_run.to_string());
    let other = call(vm.admit(ordinary, input, task_budget()).unwrap());
    let boundary = vm.defer(other.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    assert_eq!(
        vm.reserve_dependent_worker(other.continuation, child_run)
            .unwrap_err()
            .failure,
        VmFailure::WrongBoundary
    );
    assert_eq!(
        vm.reserve_dependent_worker(other.continuation, other_run)
            .unwrap_err()
            .failure,
        VmFailure::InvalidInputs
    );
    assert_eq!(
        vm.reserve_dependent_worker(other.continuation, uuid::Uuid::nil())
            .unwrap_err()
            .failure,
        VmFailure::InvalidInputs
    );
    vm.release_dependent_worker(worker, None).unwrap();
    let retained = vm.abandon();
    assert!(retained.contains(&parent.continuation));
    assert!(retained.contains(&other.continuation));
}

#[test]
fn dependent_slot_release_requires_real_finish_ack_and_keeps_parent_wait_intact() {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source-file");
    let output = directory.path().join("actual-completion");
    let content = "real child data: ü 'quoted' {{not_source}}\n";
    std::fs::write(&input, content).unwrap();
    let source = POOL_ROOT.replace(
        "        await host.resolve_intent(task['task_token'], task['user_input'])\n        raise RuntimeError('this fixture never fabricates a successful task result')",
        "        data = await host.read_file(task['task_token'], task['user_input'])\n        reference = await host.persist_result(task['task_token'], data)\n        await host.finish_task(task['task_token'], {'status': 'internal_completed', 'receipt_ref': reference})\n        task = None\n        data = None\n        reference = None",
    );
    let mut ports = aliases();
    ports.extend([
        "await_pool_update".into(),
        "read_file".into(),
        "persist_result".into(),
    ]);
    let mut vm = GlobalVm::start_ready(
        Arc::from(source.as_str()),
        Sha256::digest(source.as_bytes()).into(),
        ports,
        GlobalBounds {
            workers: 1,
            ..bounds()
        },
        Duration::from_secs(5),
    )
    .unwrap();
    let identity = vm.execution_identity();
    let parent = call(
        vm.admit(
            vm.work_waits()[0].1,
            task("file-parent", input.to_str().unwrap()),
            task_budget(),
        )
        .unwrap(),
    );
    assert_eq!(parent.name, "read_file");
    let boundary = vm.defer(parent.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    let child_run = uuid::Uuid::new_v4();
    let (worker, boundary) = vm
        .reserve_dependent_worker(parent.continuation, child_run)
        .unwrap();
    settle_pool(&mut vm, boundary);
    let mut child = task("file-child", input.to_str().unwrap());
    child["run_id"] = json!(child_run.to_string());
    let read = call(
        vm.admit(
            vm.dependent_worker_wait(worker).unwrap(),
            child,
            task_budget(),
        )
        .unwrap(),
    );
    assert_eq!(read.name, "read_file");
    assert_eq!(
        read.args,
        [json!("file-child"), json!(input.to_str().unwrap())]
    );
    let boundary = vm.defer(read.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    let actual = std::fs::read_to_string(&input).unwrap();
    let persist = call(
        vm.resolve(read.continuation, HostAnswer::Return(json!(actual).into()))
            .unwrap(),
    );
    assert_eq!(persist.name, "persist_result");
    assert_eq!(persist.args, [json!("file-child"), json!(content)]);
    let boundary = vm.defer(persist.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap();
    file.write_all(persist.args[1].as_str().unwrap().as_bytes())
        .unwrap();
    file.sync_all().unwrap();
    let reference = format!("file:{}", output.display());
    let finish = call(
        vm.resolve(
            persist.continuation,
            HostAnswer::Return(json!(reference).into()),
        )
        .unwrap(),
    );
    assert_eq!(finish.name, "finish_task");
    assert_eq!(
        finish.args[1],
        json!({"status":"internal_completed", "receipt_ref":reference})
    );
    let boundary = vm.defer(finish.continuation).unwrap();
    settle_pool(&mut vm, boundary);
    assert_eq!(
        vm.release_dependent_worker(worker, Some("file-child"))
            .unwrap_err()
            .failure,
        VmFailure::WrongBoundary
    );
    let before = vm.outstanding();
    let parent_data = std::fs::read_to_string(&input).unwrap();
    let held = vm
        .resolve(
            parent.continuation,
            HostAnswer::Return(json!(parent_data).into()),
        )
        .unwrap_err();
    assert_eq!(held.failure, VmFailure::WrongBoundary);
    assert_eq!(vm.outstanding(), before);
    let held_parent_answer = *held
        .rejected_answer
        .expect("retain the actual completed read");
    // The file is real, synced completion evidence. Only after checking its
    // exact receipt does this fixture owner acknowledge the finish port.
    assert_eq!(std::fs::read_to_string(&output).unwrap(), content);
    let boundary = vm
        .resolve(finish.continuation, HostAnswer::Return(Value::Null.into()))
        .unwrap();
    settle_pool(&mut vm, boundary);
    assert!(!vm.task_active("file-child").unwrap());
    for token in [None, Some("another-child")] {
        assert_eq!(
            vm.release_dependent_worker(worker, token)
                .unwrap_err()
                .failure,
            VmFailure::WrongBoundary
        );
    }
    let waiting = vm.dependent_worker_wait(worker).unwrap();
    vm.release_dependent_worker(worker, Some("file-child"))
        .unwrap();
    let boundary = vm.drain_worker(waiting).unwrap();
    settle_pool(&mut vm, boundary);
    assert!(vm.worker_pool_capacity().unwrap().settled());
    assert_eq!(vm.worker_pool_capacity().unwrap().active_ids, [0]);
    assert_eq!(vm.execution_identity(), identity);
    assert!(vm.outstanding().contains(&parent.continuation));
    let parent_persist = call(vm.resolve(parent.continuation, held_parent_answer).unwrap());
    assert_eq!(parent_persist.name, "persist_result");
    assert_eq!(parent_persist.args, [json!("file-parent"), json!(content)]);
    // The parent continued with its original read exactly after release. Its
    // next effect has not been executed; preserve that new continuation.
    assert!(vm.abandon().contains(&parent_persist.continuation));
}
