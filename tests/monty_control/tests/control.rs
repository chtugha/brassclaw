use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use brassclaw_resources::{
    LiveMontyTaskSettings, MontyTaskBudgetError, MontyTaskClock, MontyTaskLimits,
    MontyTaskSettingsRevision, SharedMontyTaskBudget,
};
use monty::{Dump, MontyRepl, MontyRun, ReplProgress, RunProgress, Session, SessionRef, dump};
use monty_types::{
    CompileOptions, ExecutionControl, ExecutionControlAction, ExecutionControlError, MontyObject,
    PrintWriter, ResourceLimits, ResourceTracker,
};

#[derive(Debug, Default)]
struct Control {
    yield_once: AtomicBool,
    cancel_after: AtomicUsize,
    calls: AtomicUsize,
    elapsed: Mutex<Duration>,
}
impl ExecutionControl for Control {
    fn checkpoint(
        &self,
        elapsed: Duration,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        let mut last = self.elapsed.lock().unwrap();
        assert!(elapsed >= *last);
        *last = elapsed;
        let calls = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let cancel_after = self.cancel_after.load(Ordering::SeqCst);
        if cancel_after != 0 && calls >= cancel_after {
            return Err(ExecutionControlError::Cancelled);
        }
        Ok(if self.yield_once.swap(false, Ordering::SeqCst) {
            ExecutionControlAction::Yield
        } else {
            ExecutionControlAction::Continue
        })
    }
}
fn tracker(control: Arc<dyn ExecutionControl>) -> ResourceTracker {
    let mut tracker =
        ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_secs(5)));
    tracker.set_execution_control(control);
    tracker
}
fn program(code: &str) -> MontyRun {
    MontyRun::new(code.into(), "control.py", vec![], CompileOptions::default()).unwrap()
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

#[test]
fn pure_loop_yields_without_replaying_or_injecting_a_value() {
    let control = Arc::new(Control::default());
    control.yield_once.store(true, Ordering::SeqCst);
    let RunProgress::ControlYield(paused) =
        program("total = 0\nfor i in range(10000):\n    total += i\ntotal")
            .start(vec![], tracker(control.clone()), PrintWriter::Disabled)
            .unwrap()
    else {
        panic!("busy Python must yield to the host");
    };
    let before = paused.tracker().elapsed();
    // Another real interpreter runs while this continuation is parked.
    let RunProgress::Complete(other) = program("40 + 2")
        .start(vec![], ResourceTracker::default(), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("independent task should complete");
    };
    assert_eq!(other, MontyObject::int(42));
    assert_eq!(paused.tracker().elapsed(), before);
    let RunProgress::Complete(value) = paused.resume(PrintWriter::Disabled).unwrap() else {
        panic!("the same loop must finish");
    };
    assert_eq!(value, MontyObject::int(49_995_000));
    assert!(*control.elapsed.lock().unwrap() > before);
}

#[test]
fn repl_resume_preserves_locals_exception_state_and_later_feeds() {
    let control = Arc::new(Control::default());
    control.yield_once.store(true, Ordering::SeqCst);
    let repl = MontyRepl::new("recipe.py", tracker(control), CompileOptions::default());
    let ReplProgress::ControlYield(paused) = repl.feed_start(
        "def calculate():\n    total = 0\n    try:\n        for i in range(10000):\n            total += i\n        raise ValueError('expected')\n    except ValueError:\n        return total\nanswer = calculate()\nanswer",
        vec![], PrintWriter::Disabled,
    ).unwrap() else { panic!("must suspend inside the Recipe's function"); };
    let ReplProgress::Complete { repl, value } = paused.resume(PrintWriter::Disabled).unwrap()
    else {
        panic!("must complete after handling the Python exception");
    };
    assert_eq!(value, MontyObject::int(49_995_000));
    let ReplProgress::Complete { value, .. } = repl
        .feed_start("answer + 1", vec![], PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("next step must retain Recipe state");
    };
    assert_eq!(value, MontyObject::int(49_995_001));
}

#[test]
fn cancellation_inside_a_busy_try_block_is_uncatchable() {
    let control = Arc::new(Control::default());
    control.cancel_after.store(4, Ordering::SeqCst);
    let error = program("try:\n    while True:\n        pass\nexcept BaseException:\n    123")
        .start(vec![], tracker(control.clone()), PrintWriter::Disabled)
        .unwrap_err();
    assert!(error.to_string().contains("Cancelled"));
    assert!(*control.elapsed.lock().unwrap() > Duration::ZERO);
}

#[derive(Debug)]
struct LiveBudgetControl {
    clock: Mutex<MontyTaskClock>,
    live: LiveMontyTaskSettings,
    calls: AtomicUsize,
}
impl ExecutionControl for LiveBudgetControl {
    fn checkpoint(
        &self,
        elapsed: Duration,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        // Publish while the actual bytecode loop is executing. The next budget
        // check uses the same live revision as Rust, with no clock reset.
        if self.calls.fetch_add(1, Ordering::SeqCst) == 3 {
            self.live.publish(1, settings(2, 30)).unwrap();
        }
        self.clock
            .lock()
            .unwrap()
            .checkpoint(elapsed)
            .map_err(|error| match error {
                MontyTaskBudgetError::ComputeExceeded => ExecutionControlError::TaskComputeExceeded,
                _ => ExecutionControlError::AccountingUnavailable,
            })?;
        Ok(ExecutionControlAction::Continue)
    }
}
#[test]
fn live_revision_interrupts_busy_python_and_preserves_prior_task_usage() {
    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let budget = SharedMontyTaskBudget::new(live.clone());
    // Actual earlier active segments are represented by the retained task
    // account, independent of this interpreter's fresh cumulative clock.
    budget.record_compute_time(Duration::from_secs(31)).unwrap();
    let control = Arc::new(LiveBudgetControl {
        clock: Mutex::new(budget.execution_clock(Duration::ZERO)),
        live: live.clone(),
        calls: AtomicUsize::new(0),
    });
    let error = program("while True:\n    pass")
        .start(vec![], tracker(control), PrintWriter::Disabled)
        .unwrap_err();
    assert!(error.to_string().contains("TaskComputeExceeded"));
    assert_eq!(live.current(), settings(2, 30));
    assert_eq!(budget.check(), Err(MontyTaskBudgetError::ComputeExceeded));
    live.publish(2, settings(3, 600)).unwrap();
    assert_eq!(budget.check(), Err(MontyTaskBudgetError::ComputeExceeded));
}

#[test]
fn completed_and_failed_short_feeds_report_the_final_execution_window() {
    for code in ["1 + 2", "raise ValueError('failure')"] {
        let control = Arc::new(Control::default());
        let result = program(code).start(vec![], tracker(control.clone()), PrintWriter::Disabled);
        assert_eq!(result.is_ok(), code == "1 + 2");
        assert!(*control.elapsed.lock().unwrap() > Duration::ZERO);
    }
}

#[test]
fn controlled_dumps_require_reattachment_before_execution() {
    let control = Arc::new(Control::default());
    control.yield_once.store(true, Ordering::SeqCst);
    let progress = program("total = 0\nfor i in range(1000):\n    total += i\ntotal")
        .start(vec![], tracker(control), PrintWriter::Disabled)
        .unwrap();
    let bytes = dump("control.py", None, SessionRef::Running(&progress)).unwrap();
    let Session::Running(state) = Dump::load(&bytes).unwrap().state else {
        panic!("must restore running state");
    };
    let RunProgress::ControlYield(paused) = *state else {
        panic!("must restore control suspension");
    };
    let error = paused.resume(PrintWriter::Disabled).unwrap_err();
    assert!(error.to_string().contains("AccountingUnavailable"));
    let Session::Running(state) = Dump::load(&bytes).unwrap().state else {
        panic!("must restore running state");
    };
    let RunProgress::ControlYield(mut paused) = *state else {
        panic!("must restore control suspension");
    };
    paused
        .tracker_mut()
        .set_execution_control(Arc::new(Control::default()));
    let RunProgress::Complete(value) = paused.resume(PrintWriter::Disabled).unwrap() else {
        panic!("trusted reattachment must permit exact continuation");
    };
    assert_eq!(value, MontyObject::int(499_500));
}

#[test]
fn native_callbacks_defer_yield_until_the_rust_stack_has_returned() {
    let control = Arc::new(Control::default());
    control.yield_once.store(true, Ordering::SeqCst);
    let progress = program(
        "def key(value):\n    total = 0\n    for i in range(1000):\n        total += i\n    return -value\nvalues = sorted(range(100), key=key)\nfor i in range(1000):\n    pass\nvalues[0]",
    ).start(vec![], tracker(control), PrintWriter::Disabled).unwrap();
    let RunProgress::ControlYield(paused) = progress else {
        panic!("native callback must return before suspension");
    };
    let RunProgress::Complete(value) = paused.resume(PrintWriter::Disabled).unwrap() else {
        panic!("native result and caller locals must survive");
    };
    assert_eq!(value, MontyObject::int(99));
}

#[test]
fn repl_compilation_is_charged_on_syntax_failure_and_before_any_opcode() {
    let control = Arc::new(Control::default());
    let mut repl = MontyRepl::new(
        "preparation.py",
        tracker(control.clone()),
        CompileOptions::default(),
    );
    repl.feed_run(
        "def original():\n    return 41",
        vec![],
        PrintWriter::Disabled,
    )
    .unwrap();
    assert!(repl.has_function("original"));
    let before_syntax = repl.tracker().preparation_elapsed().unwrap();
    let before_vm = repl.tracker().elapsed();
    assert!(
        repl.feed_run("def broken(:", vec![], PrintWriter::Disabled)
            .is_err()
    );
    assert!(repl.tracker().preparation_elapsed().unwrap() > before_syntax);
    assert_eq!(repl.tracker().elapsed(), before_vm);
    assert!(repl.has_function("original"));

    // The four preparation checkpoints are scan entry/exit and compile
    // entry/exit. Cancel after successful compilation has moved the tables.
    let checkpoints = control.calls.load(Ordering::SeqCst);
    control
        .cancel_after
        .store(checkpoints + 4, Ordering::SeqCst);
    let mut stdout = String::new();
    let error = repl
        .feed_start(
            "new_name = 99\nprint('must not execute')\noriginal()",
            vec![],
            PrintWriter::CollectString(&mut stdout, None),
        )
        .unwrap_err();
    assert!(error.error.to_string().contains("Cancelled"));
    assert!(stdout.is_empty());
    assert_eq!(error.repl.tracker().elapsed(), before_vm);
    assert!(error.repl.tracker().preparation_elapsed().unwrap() > before_syntax);
    assert!(error.repl.has_function("original"));
    let mut retained = error.repl;
    retained
        .tracker_mut()
        .set_execution_control(Arc::new(Control::default()));
    assert!(
        retained
            .feed_run("original()", vec![], PrintWriter::Disabled)
            .unwrap_err()
            .to_string()
            .contains("Cancelled")
    );
}

#[test]
fn preparation_clock_survives_dumps_without_resetting_or_accepting_old_abi() {
    let control = Arc::new(Control::default());
    let mut repl = MontyRepl::new(
        "preparation.py",
        tracker(control),
        CompileOptions::default(),
    );
    repl.feed_run("answer = 42", vec![], PrintWriter::Disabled)
        .unwrap();
    let preparation = repl.tracker().preparation_elapsed().unwrap();
    let execution = repl.tracker().elapsed();
    let bytes = dump("preparation.py", None, SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut loaded) = Dump::load(&bytes).unwrap().state else {
        panic!("must restore idle REPL");
    };
    assert_eq!(loaded.tracker().preparation_elapsed().unwrap(), preparation);
    assert_eq!(loaded.tracker().elapsed(), execution);
    let reattached = Arc::new(Control::default());
    loaded
        .tracker_mut()
        .set_execution_control(reattached.clone());
    assert_eq!(
        loaded
            .feed_run("answer", vec![], PrintWriter::Disabled)
            .unwrap(),
        MontyObject::int(42)
    );
    assert!(loaded.tracker().preparation_elapsed().unwrap() > preparation);
    assert_eq!(
        *reattached.elapsed.lock().unwrap(),
        loaded.tracker().elapsed() + loaded.tracker().preparation_elapsed().unwrap()
    );
    for previous in [0xBC01_u16, 0xBC02_u16, 0xBC03_u16, 0xBC04_u16] {
        let mut old_abi = bytes.clone();
        assert_eq!(&old_abi[..6], b"MONTY\0");
        old_abi[6..8].copy_from_slice(&previous.to_le_bytes());
        assert!(Dump::load(&old_abi).is_err());
    }
}

#[test]
fn live_shared_task_limit_is_checked_at_compiler_exit_before_python_effects() {
    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let shared = SharedMontyTaskBudget::new(live.clone());
    shared.record_compute_time(Duration::from_secs(31)).unwrap();
    let control = Arc::new(LiveBudgetControl {
        clock: Mutex::new(shared.execution_clock(Duration::ZERO)),
        live: live.clone(),
        calls: AtomicUsize::new(0),
    });
    // Here checkpoint four is compiler exit, after scan entry/exit and
    // compiler entry. The real live publication must reject the compiled feed.
    let repl = MontyRepl::new(
        "preparation.py",
        tracker(control),
        CompileOptions::default(),
    );
    let mut stdout = String::new();
    let error = repl
        .feed_start(
            "try:\n    print('must not execute')\nexcept BaseException:\n    print('caught')",
            vec![],
            PrintWriter::CollectString(&mut stdout, None),
        )
        .unwrap_err();
    assert!(error.error.to_string().contains("TaskComputeExceeded"));
    assert!(error.repl.tracker().preparation_elapsed().unwrap() > Duration::ZERO);
    assert_eq!(error.repl.tracker().elapsed(), Duration::ZERO);
    assert!(stdout.is_empty());
    assert_eq!(live.current(), settings(2, 30));
    assert_eq!(shared.check(), Err(MontyTaskBudgetError::ComputeExceeded));
    live.publish(2, settings(3, 600)).unwrap();
    assert_eq!(shared.check(), Err(MontyTaskBudgetError::ComputeExceeded));
}

#[test]
fn input_and_completed_child_return_import_fail_before_python_can_catch_or_dispatch() {
    let control = Arc::new(Control::default());
    // Scan and compile use four checkpoints; scalar input import uses entry
    // and exit next. Cancellation at its exit must execute no Python opcode.
    control.cancel_after.store(6, Ordering::SeqCst);
    let repl = MontyRepl::new("import.py", tracker(control), CompileOptions::default());
    let mut stdout = String::new();
    let error = repl
        .feed_start(
            "print('must not execute')\ndata",
            vec![("data".into(), MontyObject::int(42))],
            PrintWriter::CollectString(&mut stdout, None),
        )
        .unwrap_err();
    assert!(error.error.to_string().contains("Cancelled"));
    assert_eq!(error.repl.tracker().elapsed(), Duration::ZERO);
    assert!(error.repl.tracker().preparation_elapsed().unwrap() > Duration::ZERO);
    assert!(stdout.is_empty());

    let control = Arc::new(Control::default());
    let repl = MontyRepl::new(
        "return.py",
        tracker(control.clone()),
        CompileOptions::default(),
    );
    let ReplProgress::FunctionCall(call) = repl.feed_start(
        "try:\n    result = child()\n    print('after')\nexcept BaseException:\n    print('caught')",
        vec![], PrintWriter::CollectString(&mut stdout, None),
    ).unwrap() else { panic!("parent must suspend for the child") };
    let RunProgress::Complete(child_result) = program("40 + 2")
        .start(vec![], ResourceTracker::default(), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("actual child must complete")
    };
    control
        .cancel_after
        .store(control.calls.load(Ordering::SeqCst) + 2, Ordering::SeqCst);
    let error = call
        .resume(
            child_result.clone(),
            PrintWriter::CollectString(&mut stdout, None),
        )
        .unwrap_err();
    assert!(error.error.to_string().contains("Cancelled"));
    assert!(stdout.is_empty());
    assert_eq!(child_result, MontyObject::int(42));
}

#[test]
fn owned_preparation_retains_unwinding_and_excludes_real_native_sleep() {
    let resource = tracker(Arc::new(Control::default()));
    let started = std::time::Instant::now();
    let window = resource.preparation_window().unwrap();
    assert!(resource.boundary_window().unwrap().is_none());
    resource.sandbox_sleep(Duration::from_millis(40));
    window.finish(&resource).unwrap();
    let wall = started.elapsed();
    let charged = resource.preparation_elapsed().unwrap();
    assert!(wall >= charged + Duration::from_millis(35));
    let before = charged;
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _window = resource.preparation_window().unwrap();
        panic!("unwind the preparation segment");
    }));
    assert!(unwind.is_err());
    assert!(resource.preparation_elapsed().unwrap() > before);
    resource
        .preparation_window()
        .unwrap()
        .finish(&resource)
        .unwrap();
    let foreign = ResourceTracker::default();
    assert!(
        resource
            .preparation_window()
            .unwrap()
            .finish(&foreign)
            .is_err()
    );
    assert!(resource.preparation_elapsed().is_err());
}

#[test]
fn dumping_an_active_preparation_cannot_restore_an_idle_executable_account() {
    let repl = MontyRepl::new(
        "active.py",
        ResourceTracker::default(),
        CompileOptions::default(),
    );
    let window = repl.tracker().preparation_window().unwrap();
    let bytes = dump("active.py", None, SessionRef::Idle(&repl)).unwrap();
    window.finish(repl.tracker()).unwrap();
    let Session::Idle(mut loaded) = Dump::load(&bytes).unwrap().state else {
        panic!("must decode idle envelope")
    };
    assert!(loaded.tracker().preparation_elapsed().is_err());
    let mut stdout = String::new();
    assert!(
        loaded
            .feed_run(
                "print('must not execute')",
                vec![],
                PrintWriter::CollectString(&mut stdout, None)
            )
            .unwrap_err()
            .to_string()
            .contains("AccountingUnavailable")
    );
    assert!(stdout.is_empty());
}

#[test]
fn actual_child_graph_import_and_parent_export_charge_preparation_without_another_compile() {
    let control = Arc::new(Control::default());
    let repl = MontyRepl::new(
        "graphs.py",
        tracker(control.clone()),
        CompileOptions::default(),
    );
    let progress = repl
        .feed_start("result = child()\nresult", vec![], PrintWriter::Disabled)
        .unwrap();
    let before = progress.tracker().preparation_elapsed().unwrap();
    let ReplProgress::FunctionCall(call) = progress else {
        panic!("parent must wait for the actual child")
    };
    let RunProgress::Complete(child_result) = program("list(range(4096))")
        .start(vec![], ResourceTracker::default(), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("child must compute the list")
    };
    let ReplProgress::Complete { repl, value } = call
        .resume(child_result.clone(), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("parent must return the child data")
    };
    assert_eq!(value, child_result);
    assert!(repl.tracker().preparation_elapsed().unwrap() > before);
    assert_eq!(
        *control.elapsed.lock().unwrap(),
        repl.tracker().elapsed() + repl.tracker().preparation_elapsed().unwrap()
    );
}
