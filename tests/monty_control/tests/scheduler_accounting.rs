//! Actual interpreter execution: native callback reentry, coroutine switching,
//! snapshots, error handling and host waits. No product port result is faked.
use monty::{Dump, MontyRun, RunProgress, Session, SessionRef, dump};
use monty_types::{
    CompileOptions, ExecutionControl, ExecutionControlAction, ExecutionControlError,
    ExecutionObservation, MontyObject, PrintWriter, ResourceLimits, ResourceTracker,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Debug, Default)]
struct ContextControl {
    observations: Mutex<Vec<ExecutionObservation>>,
    worker_checks: AtomicUsize,
    yielded: AtomicBool,
    abort_worker: AtomicBool,
}
impl ExecutionControl for ContextControl {
    fn checkpoint(&self, _: Duration) -> Result<ExecutionControlAction, ExecutionControlError> {
        Err(ExecutionControlError::AccountingUnavailable)
    }
    fn checkpoint_context(
        &self,
        observation: ExecutionObservation,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        let mut observations = self.observations.lock().unwrap();
        if let Some(previous) = observations.last() {
            assert!(observation.execution >= previous.execution);
            assert!(observation.preparation >= previous.preparation);
        }
        observations.push(observation);
        if observation.context == Some(1) && self.abort_worker.load(Ordering::SeqCst) {
            return Err(ExecutionControlError::Cancelled);
        }
        if observation.context == Some(1)
            && self.worker_checks.fetch_add(1, Ordering::SeqCst) >= 64
            && !self.yielded.swap(true, Ordering::SeqCst)
        {
            Ok(ExecutionControlAction::Yield)
        } else {
            Ok(ExecutionControlAction::Continue)
        }
    }
}
fn tracker(control: Arc<ContextControl>) -> ResourceTracker {
    let mut tracker =
        ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_secs(5)));
    tracker.set_execution_control(control);
    tracker
}

#[test]
fn native_reentry_yield_and_dump_retain_the_actual_worker_context() {
    let source = "import asyncio\ndef increment(x):\n    checkpoint_total = 0\n    for i in range(1000):\n        checkpoint_total += i\n    if x == 63:\n        raise ValueError('native callback')\n    return x + 1\nasync def worker():\n    try:\n        total = sum(map(increment, range(64)))\n    except ValueError:\n        total = 42\n    for i in range(1000):\n        pass\n    await outside(total)\n    return total\nawait asyncio.gather(worker())";
    let control = Arc::new(ContextControl::default());
    let program = MontyRun::new(
        source.into(),
        "native-context.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let progress = program
        .start(vec![], tracker(control.clone()), PrintWriter::Disabled)
        .unwrap();
    let RunProgress::ControlYield(paused) = &progress else {
        panic!("native reentry must preserve a requested safe-boundary yield");
    };
    assert_eq!(paused.tracker().execution_context(), Some(1));
    assert!(control.yielded.load(Ordering::SeqCst));
    let execution = paused.tracker().elapsed();
    let preparation = paused.tracker().preparation_elapsed().unwrap();
    let bytes = dump("native-context.py", None, SessionRef::Running(&progress)).unwrap();
    drop(progress);
    let Session::Running(loaded) = Dump::load(&bytes).unwrap().state else {
        panic!("must restore the running coroutine");
    };
    let RunProgress::ControlYield(mut paused) = *loaded else {
        panic!("must restore the same control boundary");
    };
    assert_eq!(paused.tracker().execution_context(), Some(1));
    assert_eq!(paused.tracker().elapsed(), execution);
    assert_eq!(paused.tracker().preparation_elapsed().unwrap(), preparation);
    paused.tracker_mut().set_execution_control(control.clone());
    let RunProgress::FunctionCall(call) = paused.resume(PrintWriter::Disabled).unwrap() else {
        panic!("must reach the actual outside control call");
    };
    assert_eq!(call.function_name, "outside");
    assert_eq!(call.args.args().len(), 1);
    assert_eq!(
        call.args.args().next().unwrap().to_owned(),
        MontyObject::int(42)
    );
    assert_eq!(call.tracker().execution_context(), Some(1));
    let execution = call.tracker().elapsed();
    let preparation = call.tracker().preparation_elapsed().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(call.tracker().elapsed(), execution);
    assert_eq!(call.tracker().preparation_elapsed().unwrap(), preparation);
    assert!(call.allow_eager_await);
    let RunProgress::Complete(value) = call
        .resume_eager(Ok(MontyObject::none()), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("worker and gather must complete");
    };
    assert_eq!(value, MontyObject::list(vec![MontyObject::int(42)]));
    let observations = control.observations.lock().unwrap();
    assert!(observations.iter().any(|sample| sample.context.is_none()));
    assert_eq!(observations.last().unwrap().context, Some(0));
    assert!(observations.last().unwrap().execution > execution);
}

#[test]
fn fatal_transition_failure_prevents_even_a_short_worker_from_dispatching() {
    let control = Arc::new(ContextControl::default());
    control.abort_worker.store(true, Ordering::SeqCst);
    let source = "import asyncio\nasync def worker():\n    outside('must not dispatch')\ntry:\n    await asyncio.gather(worker())\nexcept BaseException:\n    outside('must not catch fatal control')";
    let program = MontyRun::new(
        source.into(),
        "failed-transition.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let error = program
        .start(vec![], tracker(control.clone()), PrintWriter::Disabled)
        .unwrap_err();
    assert!(error.to_string().contains("Cancelled"));
    assert!(
        control
            .observations
            .lock()
            .unwrap()
            .iter()
            .any(|sample| sample.context == Some(1))
    );
}

#[test]
fn short_success_and_error_publish_the_final_main_context_clock() {
    for source in ["1 + 2", "raise ValueError('test failure')"] {
        let control = Arc::new(ContextControl::default());
        let program = MontyRun::new(
            source.into(),
            "short-context.py",
            vec![],
            CompileOptions::default(),
        )
        .unwrap();
        let result = program.start(vec![], tracker(control.clone()), PrintWriter::Disabled);
        assert_eq!(result.is_ok(), source == "1 + 2");
        let observations = control.observations.lock().unwrap();
        let last = observations.last().unwrap();
        assert_eq!(last.context, Some(0));
        assert!(last.execution > Duration::ZERO);
        assert!(last.preparation > Duration::ZERO);
    }
}
