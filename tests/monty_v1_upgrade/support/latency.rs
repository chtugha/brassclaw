use std::time::Duration;

pub fn report(version: &str, mut execute: impl FnMut() -> Duration) {
    for _ in 0..10 {
        execute();
    }
    const SAMPLES: usize = 200;
    let mut samples: Vec<_> = (0..SAMPLES).map(|_| execute()).collect();
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(SAMPLES * percent).div_ceil(100) - 1].as_nanos();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "monty_interpreter_baseline version={version} samples={SAMPLES} profile={profile} workload=parse_and_execute_1500_integer_additions p50_ns={} p95_ns={}",
        percentile(50),
        percentile(95)
    );
}
