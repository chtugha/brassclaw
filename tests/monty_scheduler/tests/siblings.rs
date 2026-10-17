use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use monty::{Dump, MontyRepl, MontyRun, ReplProgress, RunProgress, Session, SessionRef, dump};
use monty_types::{
    CompileOptions, ExecutionControl, ExecutionControlAction, ExecutionControlError,
    ExecutionObservation, ExtFunctionResult, MontyObject, PrintWriter, ResourceLimits,
    ResourceTracker,
};

#[derive(Debug, Default)]
struct ExitControl {
    current: Mutex<Option<u32>>,
    exited: Mutex<BTreeSet<u32>>,
    reject: bool,
}

impl ExecutionControl for ExitControl {
    fn checkpoint(&self, _: Duration) -> Result<ExecutionControlAction, ExecutionControlError> {
        Err(ExecutionControlError::AccountingUnavailable)
    }

    fn checkpoint_context(
        &self,
        observation: ExecutionObservation,
    ) -> Result<ExecutionControlAction, ExecutionControlError> {
        *self.current.lock().unwrap() = observation.context;
        Ok(ExecutionControlAction::Continue)
    }

    fn context_exited(&self, context: u32) -> Result<(), ExecutionControlError> {
        assert_ne!(*self.current.lock().unwrap(), Some(context));
        assert!(self.exited.lock().unwrap().insert(context));
        if self.reject {
            Err(ExecutionControlError::AccountingUnavailable)
        } else {
            Ok(())
        }
    }
}

fn run(source: &str) -> MontyObject {
    let run = MontyRun::new(
        source.into(),
        "scheduler-siblings.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let RunProgress::Complete(result) = run
        .start(
            vec![],
            ResourceTracker::new(
                ResourceLimits::default().max_feed_duration(Duration::from_secs(10)),
            ),
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("pure scheduler case must finish without a host callback")
    };
    result
}

#[test]
fn completed_sibling_slots_are_reused_without_growing_the_result_group() {
    let result = run(r#"
import asyncio
async def worker(value):
    return value
async def barrier():
    return None
async def coordinator():
    await asyncio.gather(barrier())
    for value in range(2000):
        asyncio._replace_sibling(1, worker(value))
        await asyncio.gather(barrier())
    return 'coordinator'
await asyncio.gather(coordinator(), worker(-1))
"#);
    assert_eq!(
        result,
        MontyObject::list(vec![
            MontyObject::string("coordinator"),
            MontyObject::int(1999)
        ])
    );
}

#[test]
fn new_slot_extends_the_same_group_and_its_original_waiter() {
    let result = run(r#"
import asyncio
async def worker(value):
    return value
async def coordinator():
    asyncio._replace_sibling(1, worker('first'))
    asyncio._replace_sibling(2, worker('second'))
    return 'coordinator'
await asyncio.gather(coordinator())
"#);
    assert_eq!(
        result,
        MontyObject::list(vec![
            MontyObject::string("coordinator"),
            MontyObject::string("first"),
            MontyObject::string("second"),
        ])
    );
}

#[test]
fn rejected_replacements_preserve_running_siblings_and_never_start_rejected_code() {
    let result = run(r#"
import asyncio
started = []
async def forbidden():
    started.append('forbidden')
async def sibling():
    started.append('sibling')
    return 42
async def coordinator():
    errors = []
    for slot in [0, 1, 3, -1, True, '1']:
        try:
            asyncio._replace_sibling(slot, forbidden())
        except Exception as error:
            errors.append(str(error))
    return errors
results = await asyncio.gather(coordinator(), sibling())
assert started == ['sibling']
assert results[1] == 42
results[0]
"#);
    assert_eq!(
        result,
        MontyObject::list(
            [
                "gather slot is still running",
                "gather slot is still running",
                "gather slot is outside the group",
                "gather slot must be a nonnegative integer",
                "gather slot must be a nonnegative integer",
                "gather slot must be a nonnegative integer",
            ]
            .into_iter()
            .map(MontyObject::string)
        )
    );
}

#[test]
fn no_group_or_non_coroutine_never_creates_a_detached_task() {
    assert_eq!(
        run(r#"
import asyncio
async def worker():
    raise RuntimeError('must not run')
try:
    asyncio._replace_sibling(0, worker())
except RuntimeError as error:
    result = str(error)
assert result == 'no executing gather child'
async def coordinator():
    try:
        asyncio._replace_sibling(1, 42)
    except TypeError as error:
        return str(error)
await asyncio.gather(coordinator())
"#,),
        MontyObject::list(vec![MontyObject::string("a new coroutine is required")])
    );
}

#[test]
fn newly_spawned_failure_uses_the_existing_group_error_path() {
    assert_eq!(
        run(r#"
import asyncio
async def broken():
    raise ValueError('worker failed')
async def coordinator():
    asyncio._replace_sibling(1, broken())
    return None
try:
    await asyncio.gather(coordinator())
except ValueError as error:
    result = str(error)
result
"#,),
        MontyObject::string("worker failed")
    );
}

#[test]
fn replaced_group_retains_the_actual_host_wait_and_snapshot_without_old_tasks() {
    let program = MontyRun::new(
        r#"
import asyncio
async def worker(value):
    return value
async def barrier():
    return None
async def coordinator():
    await asyncio.gather(barrier())
    for value in range(2000):
        asyncio._replace_sibling(1, worker(value))
        await asyncio.gather(barrier())
    return await checkpoint()
await asyncio.gather(coordinator(), worker(-1))
"#
        .into(),
        "scheduler-snapshot.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let RunProgress::FunctionCall(call) = program
        .start(vec![], ResourceTracker::default(), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("must retain the actual coordinator checkpoint")
    };
    assert_eq!(call.function_name, "checkpoint");
    let call_id = call.call_id;
    let RunProgress::ResolveFutures(wait) = call.resume_pending(PrintWriter::Disabled).unwrap()
    else {
        panic!("must wait for the real external future")
    };
    assert_eq!(wait.__live_task_count_for_tests(), 2);
    let progress = RunProgress::ResolveFutures(wait);
    let bytes = dump(
        "scheduler-snapshot.py",
        None,
        SessionRef::Running(&progress),
    )
    .unwrap();
    drop(progress);
    let Session::Running(restored) = Dump::load(&bytes).unwrap().state else {
        panic!("must restore the retained scheduler")
    };
    let RunProgress::ResolveFutures(wait) = *restored else {
        panic!("must keep the coordinator's existing wait")
    };
    assert_eq!(wait.__live_task_count_for_tests(), 2);
    let RunProgress::Complete(result) = wait
        .resume(
            vec![(
                call_id,
                ExtFunctionResult::Return(MontyObject::string("confirmed")),
            )],
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("the original group must finish after the checkpoint answer")
    };
    assert_eq!(
        result,
        MontyObject::list(vec![
            MontyObject::string("confirmed"),
            MontyObject::int(1999)
        ])
    );
}

#[test]
fn abandoning_one_shot_and_repl_snapshots_releases_owned_references_without_execution() {
    for source in [
        "value = [1, 2, 3]\ncheckpoint(value)",
        "value = [1, 2, 3]\nawait checkpoint(value)",
    ] {
        let run = MontyRun::new(
            source.into(),
            "abandon.py",
            vec![],
            CompileOptions::default(),
        )
        .unwrap();
        let progress = run
            .start(vec![], ResourceTracker::default(), PrintWriter::Disabled)
            .unwrap();
        let RunProgress::FunctionCall(call) = progress else {
            panic!("must suspend before any host effect")
        };
        if source.contains("await") {
            let progress = call.resume_pending(PrintWriter::Disabled).unwrap();
            assert!(matches!(progress, RunProgress::ResolveFutures(_)));
            drop(progress);
        } else {
            drop(call);
        }
        let repl = MontyRepl::new(
            "abandon-repl.py",
            ResourceTracker::default(),
            CompileOptions::default(),
        );
        let progress = repl
            .feed_start(source, vec![], PrintWriter::Disabled)
            .unwrap();
        let ReplProgress::FunctionCall(call) = progress else {
            panic!("must suspend the REPL before any host effect")
        };
        if source.contains("await") {
            let progress = call.resume_pending(PrintWriter::Disabled).unwrap();
            assert!(matches!(progress, ReplProgress::ResolveFutures(_)));
            drop(progress);
        } else {
            drop(call);
        }
    }
}

#[test]
fn repl_snapshot_round_trip_and_abandon_to_namespace_preserve_selected_globals() {
    let repl = MontyRepl::new(
        "retained-repl.py",
        ResourceTracker::default(),
        CompileOptions::default(),
    );
    let progress = repl
        .feed_start(
            "value = [1, 2, 3]\nawait checkpoint(value)",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap();
    let ReplProgress::FunctionCall(call) = progress else {
        panic!("must reach the checkpoint")
    };
    let progress = call.resume_pending(PrintWriter::Disabled).unwrap();
    let bytes = dump("retained-repl.py", None, SessionRef::Suspended(&progress)).unwrap();
    drop(progress);
    let Session::Suspended(restored) = Dump::load(&bytes).unwrap().state else {
        panic!("must restore the suspended REPL")
    };
    let repl = restored.into_repl();
    let ReplProgress::Complete { value, .. } = repl
        .feed_start("value", vec![], PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("abandoning the snippet must retain its namespace")
    };
    assert_eq!(
        value,
        MontyObject::list([
            MontyObject::int(1),
            MontyObject::int(2),
            MontyObject::int(3)
        ])
    );
}

#[test]
fn context_retirement_follows_real_exits_and_never_snapshot_destruction() {
    let control = Arc::new(ExitControl::default());
    let mut tracker = ResourceTracker::default();
    tracker.set_execution_control(control.clone());
    let program = MontyRun::new(
        "import asyncio\nasync def completed():\n    return 42\nasync def waiting():\n    await checkpoint()\nawait asyncio.gather(waiting(), completed())".into(),
        "exit-observation.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let RunProgress::FunctionCall(call) = program
        .start(vec![], tracker, PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("must reach the actual external wait")
    };
    let RunProgress::ResolveFutures(wait) = call.resume_pending(PrintWriter::Disabled).unwrap()
    else {
        panic!("the completed sibling must leave the other coroutine parked")
    };
    let retired = control.exited.lock().unwrap().clone();
    assert_eq!(retired.len(), 1);
    assert_eq!(wait.__live_task_count_for_tests(), 2);
    drop(wait);
    assert_eq!(*control.exited.lock().unwrap(), retired);
}

#[test]
fn retirement_rejection_is_uncatchable_and_prevents_further_python_execution() {
    let control = Arc::new(ExitControl {
        reject: true,
        ..ExitControl::default()
    });
    let mut tracker = ResourceTracker::default();
    tracker.set_execution_control(control.clone());
    let program = MontyRun::new(
        "import asyncio\nasync def child():\n    return 42\ntry:\n    await asyncio.gather(child())\nexcept Exception:\n    checkpoint('must not execute')".into(),
        "exit-rejection.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let error = program
        .start(vec![], tracker, PrintWriter::Disabled)
        .unwrap_err();
    assert!(error.to_string().contains("AccountingUnavailable"));
    assert_eq!(control.exited.lock().unwrap().len(), 1);
}
