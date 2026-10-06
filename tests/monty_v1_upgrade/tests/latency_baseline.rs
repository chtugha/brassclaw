//! Repeatable interpreter-only diagnostic. Production p50/p95 acceptance still
//! needs end-to-end measurements with allocator, settings and kernel wiring.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const CODE: &str = "total = 0\nfor i in range(1500):\n    total += i\ntotal";
const ANSWER: i64 = 1_124_250;
#[path = "../support/latency.rs"]
mod latency;

fn upgraded() -> Duration {
    let begin = Instant::now();
    let run = monty::MontyRun::new(
        CODE.into(),
        "baseline.py",
        vec![],
        monty_types::CompileOptions::default(),
    )
    .unwrap();
    let output = run
        .start(
            vec![],
            monty_types::ResourceTracker::new(
                monty_types::ResourceLimits::default().max_feed_duration(Duration::from_secs(5)),
            ),
            monty_types::PrintWriter::Disabled,
        )
        .unwrap();
    let elapsed = begin.elapsed();
    let monty::RunProgress::Complete(value) = output else {
        panic!("unexpected suspension")
    };
    assert_eq!(black_box(value), monty_types::MontyObject::int(ANSWER));
    elapsed
}

#[test]
fn interpreter_parse_and_execute_latency_baseline() {
    latency::report("1.0.0", upgraded);
}
