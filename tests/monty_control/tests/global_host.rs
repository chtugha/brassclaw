//! Real root-VM boundaries; no successful provider, Recipe or durable finish
//! operation is simulated. Work-wait/shutdown replies are host control events.
use brassclaw_monty_host::global::{GlobalBoundary, GlobalBounds, GlobalVm, Lifecycle};
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
            max_source_bytes: 16384,
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
        "resolve_intent",
        "resolve_component_by_name",
        "compose_orchestrator",
        "run_program",
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

#[test]
fn actual_global_root_waits_at_boot_and_only_explicit_shutdown_stops_it() {
    let (mut vm, first) = start(SOURCE, bounds());
    let waits = settle_boot(&mut vm, first);
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
}

#[test]
fn opaque_admitted_tasks_interleave_and_actual_child_failure_leaves_other_waits_alive() {
    let (mut vm, first) = start(SOURCE, bounds());
    let waits = settle_boot(&mut vm, first);
    let query = "quoted '\"\\\nÜ {{vars.text}} %\nresult = host.forbidden_effect()";
    let a = call(vm.admit(waits[0], task("a", query)).unwrap());
    assert_eq!(a.name, "resolve_intent");
    assert_eq!(a.args, vec![json!("a"), json!(query)]);
    assert!(matches!(
        vm.defer(a.continuation).unwrap(),
        GlobalBoundary::Waiting(_)
    ));
    let b = call(vm.admit(waits[1], task("b", "")).unwrap());
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
    let live = LiveMontyTaskSettings::new(MontyTaskSettingsRevision {
        revision: 1,
        limits: MontyTaskLimits {
            max_compute_time: Duration::from_secs(600),
            token_budgets_enabled: false,
        },
    })
    .unwrap();
    let mut child = RecipeVm::new(SharedMontyTaskBudget::new(live), bounds().values).unwrap();
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
        .admit(waits[0], json!({"task_token": "incomplete"}))
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
        vm.admit(waits[0], extra).unwrap_err().failure,
        VmFailure::InvalidInputs
    );
    assert_eq!(vm.outstanding(), waits);
    let error = vm.admit(waits[0], task("a", &"ü".repeat(128))).unwrap_err();
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
