//! These call the actual tagged interpreter; production adapter acceptance is
//! additional work, not implied by this prerequisite suite.

use std::time::Duration;

use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExcType, ExtFunctionResult, MontyException, MontyObject, PrintWriter,
    ResourceLimits, ResourceTracker,
};

fn compile(code: &str, names: Vec<String>) -> MontyRun {
    MontyRun::new(code.into(), "upgrade.py", names, CompileOptions::default()).unwrap()
}

fn start(code: &str) -> RunProgress {
    compile(code, vec![])
        .start(
            vec![],
            ResourceTracker::new(
                ResourceLimits::default()
                    .max_feed_duration(Duration::from_secs(5))
                    .max_suspensions(128),
            ),
            PrintWriter::Disabled,
        )
        .unwrap()
}

#[test]
fn current_orchestrator_source_compiles_with_explicit_options() {
    let code = include_str!("../../../crates/brassclaw_engine/orchestrator/basic_mode.py");
    compile(code, vec![]);
}

#[test]
fn one_vm_parks_before_work_and_survives_multiple_completed_tasks() {
    let code = "while True:\n    item = await_next_turn()\n    post_reply(item)\n";
    let mut progress = start(code);
    for input in ["chat-a:one", "chat-b:two", "chat-a:three"] {
        let RunProgress::FunctionCall(wait) = progress else {
            panic!("expected work wait")
        };
        assert_eq!(wait.function_name, "await_next_turn");
        assert_eq!(wait.args.args().len(), 0);
        let next = wait
            .resume(
                ExtFunctionResult::Return(MontyObject::string(input)),
                PrintWriter::Disabled,
            )
            .unwrap();
        let RunProgress::FunctionCall(reply) = next else {
            panic!("expected reply")
        };
        assert_eq!(reply.function_name, "post_reply");
        assert_eq!(
            reply.args.args().next().unwrap().to_owned(),
            MontyObject::string(input)
        );
        progress = reply
            .resume(
                ExtFunctionResult::Return(MontyObject::none()),
                PrintWriter::Disabled,
            )
            .unwrap();
    }
    assert!(matches!(progress, RunProgress::FunctionCall(_)));
}

#[test]
fn bound_function_arguments_and_dependent_results_survive_new_graph_types() {
    let run = compile(
        "parts = sweep(user_id='operator')\nresult = store(bundle=parts['bundle'])\nresult",
        vec!["sweep".into(), "store".into()],
    );
    let progress = run
        .start(
            vec![
                MontyObject::function("sweep", None),
                MontyObject::function("store", None),
            ],
            ResourceTracker::new(
                ResourceLimits::default().max_feed_duration(Duration::from_secs(5)),
            ),
            PrintWriter::Disabled,
        )
        .unwrap();
    let RunProgress::FunctionCall(sweep) = progress else {
        panic!("expected sweep")
    };
    let (key, value) = sweep.args.kwargs().next().unwrap();
    assert_eq!(key.to_owned(), MontyObject::string("user_id"));
    assert_eq!(value.to_owned(), MontyObject::string("operator"));
    let bundle = MontyObject::dict(vec![(
        MontyObject::string("bundle"),
        MontyObject::string("validated-bundle"),
    )]);
    let progress = sweep
        .resume(ExtFunctionResult::Return(bundle), PrintWriter::Disabled)
        .unwrap();
    let RunProgress::FunctionCall(store) = progress else {
        panic!("expected store")
    };
    assert_eq!(store.function_name, "store");
    assert_eq!(
        store.args.kwargs().next().unwrap().1.to_owned(),
        MontyObject::string("validated-bundle")
    );
    let progress = store
        .resume(
            ExtFunctionResult::Return(MontyObject::string("stored")),
            PrintWriter::Disabled,
        )
        .unwrap();
    assert!(
        matches!(progress, RunProgress::Complete(value) if value == MontyObject::string("stored"))
    );
}

#[test]
fn fresh_snippet_cannot_see_another_snippets_mutable_state() {
    assert!(
        matches!(start("state = {}\nstate['private'] = 7\nstate['private']"), RunProgress::Complete(value) if value == MontyObject::int(7))
    );
    assert!(
        matches!(start("state = {}\n'private' in state"), RunProgress::Complete(value) if value == MontyObject::bool(false))
    );
}

#[test]
fn host_failure_does_not_dispatch_the_following_effect() {
    let RunProgress::FunctionCall(call) = start("first()\nforbidden_after_failure()") else {
        panic!("expected host call")
    };
    assert_eq!(call.function_name, "first");
    let error = call
        .resume(
            ExtFunctionResult::Error(MontyException::new(
                ExcType::RuntimeError,
                Some("host failed".into()),
            )),
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert!(error.to_string().contains("host failed"));
}

#[test]
fn aborting_one_execution_does_not_prevent_the_next_execution() {
    let RunProgress::FunctionCall(call) = start("pause()\nforbidden_after_cancel()") else {
        panic!("expected pause")
    };
    let error = call
        .abort(
            MontyException::new(ExcType::RuntimeError, Some("task cancelled".into())),
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert!(error.to_string().contains("task cancelled"));
    assert!(
        matches!(start("result = 42\nresult"), RunProgress::Complete(value) if value == MontyObject::int(42))
    );
}

#[test]
fn stdout_collection_is_bounded_and_fails_visibly() {
    let mut stdout = String::new();
    let error = compile("print('x' * 2048)", vec![])
        .start(
            vec![],
            ResourceTracker::new(
                ResourceLimits::default().max_feed_duration(Duration::from_secs(5)),
            ),
            PrintWriter::CollectString(&mut stdout, Some(1024)),
        )
        .unwrap_err();
    assert!(stdout.len() <= 1024);
    assert!(error.to_string().to_lowercase().contains("print"));
}

#[test]
fn execution_clock_excludes_external_wait_and_setter_retains_total_usage() {
    let RunProgress::FunctionCall(mut call) =
        start("total = 0\nfor i in range(10000):\n    total += i\npause()\ntotal")
    else {
        panic!("expected pause")
    };
    let consumed = call.tracker().elapsed();
    assert!(consumed > Duration::ZERO);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        call.tracker().elapsed(),
        consumed,
        "a suspended clock must not run"
    );
    call.tracker_mut()
        .set_max_feed_duration(Duration::from_secs(1));
    assert_eq!(
        call.tracker().elapsed(),
        consumed,
        "task accounting can retain this cumulative clock"
    );
    assert_eq!(
        call.tracker().feed_elapsed(),
        Duration::ZERO,
        "the feed setter is not a task consumption reset API"
    );
    assert!(matches!(
        call.resume(
            ExtFunctionResult::Return(MontyObject::none()),
            PrintWriter::Disabled
        )
        .unwrap(),
        RunProgress::Complete(_)
    ));
}

#[test]
fn one_vm_runs_b_while_a_waits_without_replaying_a() {
    let mut progress = start(
        r#"
import asyncio
async def worker_a():
    await wait_a()
    record("a resumed")
async def worker_b():
    await wait_b()
    record("b completed")
async def main():
    await asyncio.gather(worker_a(), worker_b())
asyncio.run(main())
"#,
    );
    let mut a = None;
    let mut b = None;
    let mut records = Vec::new();
    let mut resolved_b = false;
    loop {
        progress = match progress {
            RunProgress::FunctionCall(call) => match call.function_name.as_str() {
                "wait_a" => {
                    assert!(a.replace(call.call_id).is_none());
                    call.resume_pending(PrintWriter::Disabled).unwrap()
                }
                "wait_b" => {
                    assert!(b.replace(call.call_id).is_none());
                    call.resume_pending(PrintWriter::Disabled).unwrap()
                }
                "record" => {
                    records.push(call.args.args().next().unwrap().to_owned());
                    call.resume(
                        ExtFunctionResult::Return(MontyObject::none()),
                        PrintWriter::Disabled,
                    )
                    .unwrap()
                }
                _ => panic!("unexpected host operation"),
            },
            RunProgress::ResolveFutures(waiting) => {
                let id = if !resolved_b {
                    assert!(waiting.pending_call_ids().contains(&a.unwrap()));
                    resolved_b = true;
                    b.unwrap()
                } else {
                    assert_eq!(records, vec![MontyObject::string("b completed")]);
                    a.unwrap()
                };
                waiting
                    .resume(
                        vec![(id, ExtFunctionResult::Return(MontyObject::none()))],
                        PrintWriter::Disabled,
                    )
                    .unwrap()
            }
            RunProgress::Complete(_) => break,
            _ => panic!("unexpected suspension"),
        };
    }
    assert_eq!(
        records,
        vec![
            MontyObject::string("b completed"),
            MontyObject::string("a resumed")
        ]
    );
}
