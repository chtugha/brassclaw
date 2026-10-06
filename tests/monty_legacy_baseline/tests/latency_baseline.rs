use std::{
    hint::black_box,
    time::{Duration, Instant},
};

#[path = "../../monty_v1_upgrade/support/latency.rs"]
mod latency;

const CODE: &str = "total = 0\nfor i in range(1500):\n    total += i\ntotal";
const ANSWER: i64 = 1_124_250;

fn execute() -> Duration {
    let begin = Instant::now();
    let run = monty::MontyRun::new(CODE.into(), "baseline.py", vec![]).unwrap();
    let output = run
        .start(
            vec![],
            monty::LimitedTracker::new(
                monty::ResourceLimits::new().max_duration(Duration::from_secs(5)),
            ),
            monty::PrintWriter::Disabled,
        )
        .unwrap();
    let elapsed = begin.elapsed();
    let monty::RunProgress::Complete(value) = output else {
        panic!("unexpected suspension")
    };
    assert_eq!(black_box(value), monty::MontyObject::Int(ANSWER));
    elapsed
}

#[test]
fn interpreter_parse_and_execute_latency_baseline() {
    latency::report("0.0.16", execute);
}
