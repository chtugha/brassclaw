//! Phase 3a prerequisite: one actual pinned Monty VM must permit A→B→A.
//! This proves the upstream continuation API, not production readiness.
use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExtFunctionResult, MontyObject, PrintWriter, ResourceLimits, ResourceTracker,
};

#[test]
fn one_vm_runs_b_while_a_waits_without_replaying_a() {
    let code = r#"
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
"#;
    let run = MontyRun::new(
        code.into(),
        "global-continuation.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let mut stdout = String::new();
    let mut progress = run
        .start(
            vec![],
            ResourceTracker::new(
                ResourceLimits::default().max_feed_duration(std::time::Duration::from_secs(5)),
            ),
            PrintWriter::collect_string(&mut stdout),
        )
        .unwrap();
    let mut a = None;
    let mut b = None;
    let mut records = Vec::new();
    let mut resolved_b = false;
    loop {
        progress = match progress {
            RunProgress::FunctionCall(call) => match call.function_name.as_str() {
                "wait_a" => {
                    assert!(a.replace(call.call_id).is_none(), "A must not replay");
                    call.resume_pending(PrintWriter::collect_string(&mut stdout))
                        .unwrap()
                }
                "wait_b" => {
                    assert!(b.replace(call.call_id).is_none(), "B must not replay");
                    call.resume_pending(PrintWriter::collect_string(&mut stdout))
                        .unwrap()
                }
                "record" => {
                    records.push(call.args.arg(0).expect("record argument").to_owned());
                    call.resume(
                        ExtFunctionResult::Return(MontyObject::none()),
                        PrintWriter::collect_string(&mut stdout),
                    )
                    .unwrap()
                }
                other => panic!("unexpected call: {other}"),
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
                        PrintWriter::collect_string(&mut stdout),
                    )
                    .unwrap()
            }
            RunProgress::Complete(_) => break,
            other => panic!("unexpected progress: {other:?}"),
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
