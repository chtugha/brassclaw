//! Actual 1.0 cumulative clocks feed the production neutral account. This does
//! not establish attribution between async tasks in a global VM, final/error
//! clock recovery, active-loop live control, or production host wiring.

use std::time::Duration;

use brassclaw_resources::{
    LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision, SharedMontyTaskBudget,
};
use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExtFunctionResult, MontyObject, PrintWriter, ResourceLimits, ResourceTracker,
};

fn settings(revision: u64, seconds: u64) -> MontyTaskSettingsRevision {
    MontyTaskSettingsRevision {
        revision,
        limits: MontyTaskLimits {
            max_compute_time: Duration::from_secs(seconds),
            token_budgets_enabled: false,
        },
    }
}

fn start(code: &str) -> RunProgress {
    MontyRun::new(
        code.into(),
        "task_clock.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap()
    .start(
        vec![],
        ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_secs(5))),
        PrintWriter::Disabled,
    )
    .unwrap()
}

#[test]
fn nested_execution_and_limit_changes_share_consumption_without_wait_or_double_debit() {
    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let rust = SharedMontyTaskBudget::new(live.clone());
    let vm = rust.clone();
    let mut root_clock = vm.execution_clock(Duration::ZERO);
    let RunProgress::FunctionCall(mut root) = start(
        "total = 0\nfor i in range(10000):\n    total += i\nrun_program()\nfor i in range(10000):\n    total += i\ntask_finished()",
    ) else {
        panic!("expected nested execution handoff")
    };
    assert_eq!(root.function_name, "run_program");
    let before_nested = root.tracker().elapsed();
    root_clock.checkpoint(before_nested).unwrap();
    root_clock.checkpoint(before_nested).unwrap();

    // A distinct nested interpreter executes while the root remains parked.
    // The final host boundary exposes its cumulative clock before completion;
    // the public Complete variant does not provide that clock.
    let RunProgress::FunctionCall(nested) =
        start("total = 0\nfor i in range(10000):\n    total += i\nnested_finished()")
    else {
        panic!("expected nested final clock checkpoint")
    };
    let nested_elapsed = nested.tracker().elapsed();
    assert!(nested_elapsed > Duration::ZERO);
    let mut nested_clock = vm.execution_clock(Duration::ZERO);
    nested_clock.checkpoint(nested_elapsed).unwrap();
    assert_eq!(root.tracker().elapsed(), before_nested);

    let before_wait = rust.check().unwrap().usage.compute_time;
    std::thread::sleep(Duration::from_millis(50));
    live.publish(1, settings(2, 30)).unwrap();
    root.tracker_mut()
        .set_max_feed_duration(Duration::from_secs(1));
    assert_eq!(root.tracker().feed_elapsed(), Duration::ZERO);
    assert_eq!(root.tracker().elapsed(), before_nested);
    let after_wait = root_clock.checkpoint(root.tracker().elapsed()).unwrap();
    assert_eq!(after_wait.settings.revision, 2);
    assert_eq!(after_wait.usage.compute_time, before_wait);
    assert_eq!(rust.check(), vm.check());

    let RunProgress::FunctionCall(root) = root
        .resume(
            ExtFunctionResult::Return(MontyObject::none()),
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("expected task final clock checkpoint")
    };
    assert_eq!(root.function_name, "task_finished");
    let final_root_elapsed = root.tracker().elapsed();
    let finished = root_clock.checkpoint(final_root_elapsed).unwrap();
    assert_eq!(
        finished.usage.compute_time,
        final_root_elapsed.checked_add(nested_elapsed).unwrap()
    );
    assert_eq!(rust.check(), vm.check());

    // No tracker restoration, fresh task or settings increase may clear this
    // task's consumption. Another nested feed receives its own clock cursor.
    let RunProgress::FunctionCall(second_nested) =
        start("total = 0\nfor i in range(10000):\n    total += i\nnested_finished()")
    else {
        panic!("expected second nested final checkpoint")
    };
    let second_elapsed = second_nested.tracker().elapsed();
    vm.execution_clock(Duration::ZERO)
        .checkpoint(second_elapsed)
        .unwrap();
    live.publish(2, settings(3, 120)).unwrap();
    assert_eq!(
        rust.check().unwrap().usage.compute_time,
        final_root_elapsed
            .checked_add(nested_elapsed)
            .unwrap()
            .checked_add(second_elapsed)
            .unwrap()
    );
    assert_eq!(rust.check(), vm.check());
}

#[test]
fn repl_completion_and_errors_retain_the_final_clock_for_shared_accounting() {
    use monty::{MontyRepl, ReplProgress};
    use monty_types::ExcType;

    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let rust = SharedMontyTaskBudget::new(live.clone());
    let vm = rust.clone();
    let repl = MontyRepl::new(
        "owned_program.py",
        ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_secs(5))),
        CompileOptions::default(),
    );
    let mut clock = vm.execution_clock(repl.tracker().elapsed());
    let ReplProgress::Complete { repl, value } = repl
        .feed_start(
            "total = 0\nfor i in range(10000):\n    total += i\ntotal",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("pure program must complete");
    };
    assert_eq!(value, MontyObject::int(49_995_000));
    let completed_time = repl.tracker().elapsed();
    assert!(completed_time > Duration::ZERO);
    clock.checkpoint(completed_time).unwrap();
    assert_eq!(rust.check().unwrap().usage.compute_time, completed_time);

    let error = repl
        .feed_start(
            "for i in range(10000):\n    total += i\n1 / 0",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert_eq!(error.error.exc_type(), ExcType::ZeroDivisionError);
    let failed_time = error.repl.tracker().elapsed();
    assert!(failed_time > completed_time);
    clock.checkpoint(failed_time).unwrap();
    clock.checkpoint(failed_time).unwrap();
    assert_eq!(rust.check().unwrap().usage.compute_time, failed_time);
    assert_eq!(rust.check(), vm.check());
    live.publish(1, settings(2, 120)).unwrap();
    assert_eq!(rust.check().unwrap().usage.compute_time, failed_time);
    // Recovery of counters does not permit continuing this failed Recipe.
    // The caller must terminate its task and reconcile any preceding effects.
}

#[test]
fn repl_timeout_retains_consumption_and_cannot_be_caught_to_continue_effects() {
    use monty::MontyRepl;
    use monty_types::ExcType;

    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let rust = SharedMontyTaskBudget::new(live);
    let repl = MontyRepl::new(
        "timeout_program.py",
        ResourceTracker::new(
            ResourceLimits::default().max_feed_duration(Duration::from_millis(10)),
        ),
        CompileOptions::default(),
    );
    let mut clock = rust.execution_clock(repl.tracker().elapsed());
    let error = repl.feed_start(
        "try:\n    while True:\n        pass\nexcept BaseException:\n    forbidden_after_timeout()",
        vec![], PrintWriter::Disabled,
    ).unwrap_err();
    assert_eq!(error.error.exc_type(), ExcType::TimeoutError);
    let elapsed = error.repl.tracker().elapsed();
    assert!(elapsed >= Duration::from_millis(10));
    clock.checkpoint(elapsed).unwrap();
    assert_eq!(rust.check().unwrap().usage.compute_time, elapsed);
    // Actual timeout returns an error rather than a host FunctionCall to
    // forbidden_after_timeout. Final/error time does not require a Python hook.
}
