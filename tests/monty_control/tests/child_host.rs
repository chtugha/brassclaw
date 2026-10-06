//! Actual interpreter boundaries for the isolated hosting candidate. No provider,
//! database or Tool service is replaced by a successful stub in these cases.
use std::{collections::BTreeSet, sync::Arc, time::Duration};

use brassclaw_monty_host::{HostAnswer, PythonArtifact, RecipeVm, VmBoundary, VmBounds, VmFailure};
use brassclaw_resources::{
    LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision, SharedMontyTaskBudget,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn bounds() -> VmBounds {
    VmBounds {
        max_source_bytes: 8192,
        max_compiled_source_bytes: 65536,
        max_feeds: 32,
        max_stdout_bytes: 1024,
        execution_slice: Duration::from_secs(1),
        max_value_depth: 16,
        max_value_nodes: 1024,
        max_value_bytes: 8192,
    }
}
fn settings(revision: u64, seconds: u64) -> MontyTaskSettingsRevision {
    MontyTaskSettingsRevision {
        revision,
        limits: MontyTaskLimits {
            max_compute_time: Duration::from_secs(seconds),
            token_budgets_enabled: false,
        },
    }
}
fn budget() -> SharedMontyTaskBudget {
    SharedMontyTaskBudget::new(LiveMontyTaskSettings::new(settings(1, 600)).unwrap())
}
fn artifact(code: &str, bindings: &[&str], limits: VmBounds) -> Arc<PythonArtifact> {
    Arc::new(
        PythonArtifact::new(
            Arc::from(code),
            Sha256::digest(code.as_bytes()).into(),
            bindings.iter().map(|name| (*name).to_owned()).collect(),
            limits,
        )
        .unwrap(),
    )
}
fn complete(boundary: VmBoundary) -> Value {
    let VmBoundary::Complete(value) = boundary else {
        panic!("expected completion, got {boundary:?}");
    };
    value
}
fn call(boundary: VmBoundary) -> brassclaw_monty_host::HostRequest {
    let VmBoundary::HostCall(call) = boundary else {
        panic!("expected host suspension, got {boundary:?}");
    };
    call
}

#[test]
fn step_values_remain_data_and_required_result_never_reuses_the_previous_step() {
    let mut vm = RecipeVm::new(budget(), bounds()).unwrap();
    let text = "\"'\\\nÜ {{vars.text}} %\nresult = host.unapproved()";
    let input = json!({"text": text, "large": u64::MAX, "items": [true, null, 1.5]});
    assert_eq!(
        complete(
            vm.start_step(
                artifact("saved = inputs\nresult = saved", &[], bounds()),
                input.clone()
            )
            .unwrap()
        ),
        input
    );
    assert_eq!(
        complete(
            vm.start_step(
                artifact(
                    "result = {'text': saved['text'], 'new': inputs['number']}",
                    &[],
                    bounds()
                ),
                json!({"number": 42})
            )
            .unwrap()
        ),
        json!({"text": text, "new": 42})
    );
    let error = vm
        .start_step(artifact("ignored = saved", &[], bounds()), json!({}))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::MissingResult);
    assert_eq!(
        vm.start_step(artifact("result = 7", &[], bounds()), json!({}))
            .unwrap_err()
            .failure,
        VmFailure::Terminal
    );
}

#[test]
fn actual_child_result_resumes_exact_parent_and_other_tasks_can_progress() {
    let shared = budget();
    let mut parent = RecipeVm::new(shared.clone(), bounds()).unwrap();
    let pending = call(
        parent
            .start_step(
                artifact(
                    "visits = 1\nresult = host.run_program(value=inputs['value'])\nvisits += 1",
                    &["run_program"],
                    bounds(),
                ),
                json!({"value": 41}),
            )
            .unwrap(),
    );
    assert_eq!(pending.name, "run_program");
    assert!(pending.args.is_empty());
    let child_inputs = json!({"number": pending.kwargs["value"]});
    // A real child interpreter computes the answer; the host does not invent it.
    let mut child = RecipeVm::new(shared.clone(), bounds()).unwrap();
    let answer = complete(
        child
            .start_step(
                artifact("result = inputs['number'] + 1", &[], bounds()),
                child_inputs,
            )
            .unwrap(),
    );
    let after_child = shared.check().unwrap().usage.compute_time;
    assert!(after_child > Duration::ZERO);
    let mut other = RecipeVm::new(budget(), bounds()).unwrap();
    assert_eq!(
        complete(
            other
                .start_step(
                    artifact("visits = 99\nresult = visits", &[], bounds()),
                    json!({})
                )
                .unwrap()
        ),
        json!(99)
    );
    assert_eq!(shared.check().unwrap().usage.compute_time, after_child);
    assert_eq!(
        complete(
            parent
                .resume_host(pending.continuation, HostAnswer::Return(answer))
                .unwrap()
        ),
        json!(42)
    );
    assert_eq!(
        complete(
            parent
                .start_step(artifact("result = visits", &[], bounds()), json!({}))
                .unwrap()
        ),
        json!(2)
    );
    assert!(shared.check().unwrap().usage.compute_time > after_child);
}

#[test]
fn foreign_and_stale_replies_are_retained_without_consuming_the_correct_continuation() {
    let mut a = RecipeVm::new(budget(), bounds()).unwrap();
    let mut b = RecipeVm::new(budget(), bounds()).unwrap();
    let code = artifact("result = host.run_program()", &["run_program"], bounds());
    let a_call = call(a.start_step(code.clone(), json!({})).unwrap());
    let b_call = call(b.start_step(code.clone(), json!({})).unwrap());
    let mut child = RecipeVm::new(budget(), bounds()).unwrap();
    let value = complete(
        child
            .start_step(artifact("result = 42", &[], bounds()), json!({}))
            .unwrap(),
    );
    let error = a
        .resume_host(b_call.continuation, HostAnswer::Return(value.clone()))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::ForeignContinuation);
    let Some(retained) = error.rejected_answer else {
        panic!("reply evidence lost")
    };
    let HostAnswer::Return(retained) = *retained else {
        panic!("wrong evidence type")
    };
    assert_eq!(retained, value);
    assert_eq!(
        complete(
            a.resume_host(a_call.continuation, HostAnswer::Return(value.clone()))
                .unwrap()
        ),
        value
    );
    let next = call(a.start_step(code, json!({})).unwrap());
    let error = a
        .resume_host(a_call.continuation, HostAnswer::Return(value.clone()))
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::ForeignContinuation);
    assert!(error.rejected_answer.is_some());
    assert_eq!(
        complete(
            a.resume_host(next.continuation, HostAnswer::Return(value))
                .unwrap()
        ),
        json!(42)
    );
    b.cancel();
}

#[test]
fn cancellation_releases_parked_vm_but_preserves_pending_external_identity() {
    let mut vm = RecipeVm::new(budget(), bounds()).unwrap();
    let pending = call(
        vm.start_step(
            artifact(
                "print('before')\nresult = host.run_program()",
                &["run_program"],
                bounds(),
            ),
            json!({}),
        )
        .unwrap(),
    );
    let stopped = vm.cancel();
    assert_eq!(stopped.pending_host, Some(pending.continuation));
    assert_eq!(stopped.stdout, "before\n");
    // A real failed child is late. Its exception remains diagnostic evidence.
    let mut child = RecipeVm::new(budget(), bounds()).unwrap();
    let failure = child
        .start_step(
            artifact("raise ValueError('child failed')", &[], bounds()),
            json!({}),
        )
        .unwrap_err();
    let error = vm
        .resume_host(
            pending.continuation,
            HostAnswer::Raise(*failure.exception.unwrap()),
        )
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::Terminal);
    assert!(
        error
            .rejected_answer
            .is_some_and(|answer| matches!(*answer, HostAnswer::Raise(_)))
    );
}

#[test]
fn busy_python_yields_and_cannot_catch_a_supervisor_cancellation() {
    let limits = VmBounds {
        execution_slice: Duration::from_millis(1),
        ..bounds()
    };
    let mut vm = RecipeVm::new(budget(), limits).unwrap();
    let VmBoundary::ControlYield(key) = vm
        .start_step(
            artifact(
                "try:\n    while True:\n        pass\nexcept BaseException:\n    result = 'caught'",
                &[],
                limits,
            ),
            json!({}),
        )
        .unwrap()
    else {
        panic!("busy code must yield")
    };
    vm.cancellation().request();
    let error = vm.resume_control(key).unwrap_err();
    assert_eq!(error.failure, VmFailure::Python);
    assert!(error.exception.unwrap().to_string().contains("Cancelled"));
}

#[test]
fn live_duration_update_interrupts_retained_context_without_resetting_usage() {
    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let shared = SharedMontyTaskBudget::new(live.clone());
    // Seed retained earlier compute, rather than spend 31 seconds in a test.
    shared.record_compute_time(Duration::from_secs(31)).unwrap();
    let mut vm = RecipeVm::new(shared.clone(), bounds()).unwrap();
    let pending = call(
        vm.start_step(
            artifact("result = host.run_program()", &["run_program"], bounds()),
            json!({}),
        )
        .unwrap(),
    );
    let mut completed_child = RecipeVm::new(shared.clone(), bounds()).unwrap();
    let completed_value = complete(
        completed_child
            .start_step(artifact("result = 42", &[], bounds()), json!({}))
            .unwrap(),
    );
    live.publish(1, settings(2, 30)).unwrap();
    let mut child = RecipeVm::new(shared.clone(), bounds()).unwrap();
    let failure = child
        .start_step(artifact("result = 42", &[], bounds()), json!({}))
        .unwrap_err();
    assert!(
        failure
            .exception
            .as_ref()
            .unwrap()
            .to_string()
            .contains("TaskComputeExceeded")
    );
    let parent_failure = vm
        .resume_host(
            pending.continuation,
            HostAnswer::Return(completed_value.clone()),
        )
        .unwrap_err();
    assert_eq!(parent_failure.failure, VmFailure::Python);
    let HostAnswer::Return(retained) = *parent_failure.rejected_answer.unwrap() else {
        panic!("completed child return lost after live budget failure")
    };
    assert_eq!(retained, completed_value);
    assert!(shared.check().is_err());
    assert_eq!(live.current(), settings(2, 30));
}

#[test]
fn unbound_calls_and_invalid_transport_stop_before_dispatch() {
    for (code, bindings, failure) in [
        (
            "result = host.unapproved()",
            &[][..],
            VmFailure::UnboundHostCall,
        ),
        (
            "result = host.run_program(value=host)",
            &["run_program"][..],
            VmFailure::InvalidHostArguments,
        ),
        ("result = float('inf')", &[][..], VmFailure::InvalidResult),
        ("result = 2 ** 1000", &[][..], VmFailure::InvalidResult),
        ("result = (1, 2)", &[][..], VmFailure::InvalidResult),
        ("result = {1: 'value'}", &[][..], VmFailure::InvalidResult),
    ] {
        let mut vm = RecipeVm::new(budget(), bounds()).unwrap();
        assert_eq!(
            vm.start_step(artifact(code, bindings, bounds()), json!({}))
                .unwrap_err()
                .failure,
            failure
        );
    }
    let limits = VmBounds {
        max_value_bytes: 5,
        ..bounds()
    };
    let mut vm = RecipeVm::new(budget(), limits).unwrap();
    let code = artifact("result = inputs['value']", &[], limits);
    assert_eq!(
        vm.start_step(code.clone(), json!({"value": "x"}))
            .unwrap_err()
            .failure,
        VmFailure::InvalidInputs
    );
    // A rejected input has not begun the selected step or poisoned the context.
    assert_eq!(
        complete(vm.start_step(code, json!({"value": ""})).unwrap()),
        json!("")
    );
}

#[test]
fn source_and_stdout_limits_fail_explicitly_and_do_not_replay() {
    let code: Arc<str> = Arc::from("result = 1");
    assert_eq!(
        PythonArtifact::new(code, [0; 32], BTreeSet::new(), bounds())
            .unwrap_err()
            .failure,
        VmFailure::Integrity
    );
    let bad = "result = (";
    assert_eq!(
        PythonArtifact::new(
            Arc::from(bad),
            Sha256::digest(bad.as_bytes()).into(),
            BTreeSet::new(),
            bounds()
        )
        .unwrap_err()
        .failure,
        VmFailure::Python
    );
    let limits = VmBounds {
        max_stdout_bytes: 8,
        max_feeds: 1,
        ..bounds()
    };
    let mut vm = RecipeVm::new(budget(), limits).unwrap();
    let error = vm
        .start_step(
            artifact("print('ok')\nprint('overflowing')\nresult = 1", &[], limits),
            json!({}),
        )
        .unwrap_err();
    assert_eq!(error.failure, VmFailure::Python);
    assert!(error.stdout.starts_with("ok\n"));
    let mut vm = RecipeVm::new(budget(), limits).unwrap();
    let code = artifact("result = 1", &[], limits);
    assert_eq!(
        complete(vm.start_step(code.clone(), json!({})).unwrap()),
        json!(1)
    );
    assert_eq!(
        vm.start_step(code, json!({})).unwrap_err().failure,
        VmFailure::SourceLimit
    );
}

#[test]
fn eager_async_host_reply_comes_from_an_actual_child_vm() {
    let mut vm = RecipeVm::new(budget(), bounds()).unwrap();
    let pending = call(vm.start_step(artifact(
        "import asyncio\nasync def calculate():\n    return await host.run_program()\nresult = asyncio.run(calculate())",
        &["run_program"], bounds()), json!({})).unwrap());
    let mut child = RecipeVm::new(budget(), bounds()).unwrap();
    let value = complete(
        child
            .start_step(artifact("result = 6 * 7", &[], bounds()), json!({}))
            .unwrap(),
    );
    assert_eq!(
        complete(
            vm.resume_host(pending.continuation, HostAnswer::Return(value))
                .unwrap()
        ),
        json!(42)
    );
}

#[test]
fn actual_child_domain_failure_is_catchable_only_when_the_port_classifies_it_so() {
    let parent_code = artifact(
        "try:\n    result = host.run_program()\nexcept ValueError:\n    result = 'handled'",
        &["run_program"],
        bounds(),
    );
    for catchable in [true, false] {
        let shared = budget();
        let mut parent = RecipeVm::new(shared.clone(), bounds()).unwrap();
        let pending = call(parent.start_step(parent_code.clone(), json!({})).unwrap());
        let mut child = RecipeVm::new(shared, bounds()).unwrap();
        let failure = child
            .start_step(
                artifact("raise ValueError('actual child error')", &[], bounds()),
                json!({}),
            )
            .unwrap_err();
        let exception = *failure.exception.unwrap();
        let answer = if catchable {
            HostAnswer::Raise(exception)
        } else {
            HostAnswer::Abort(exception)
        };
        let result = parent.resume_host(pending.continuation, answer);
        if catchable {
            assert_eq!(complete(result.unwrap()), json!("handled"));
        } else {
            assert_eq!(result.unwrap_err().failure, VmFailure::Python);
            assert_eq!(
                parent
                    .start_step(artifact("result = 'replayed'", &[], bounds()), json!({}),)
                    .unwrap_err()
                    .failure,
                VmFailure::Terminal
            );
        }
    }
}
