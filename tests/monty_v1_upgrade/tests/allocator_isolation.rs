use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn probe(mode: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_allocator_probe"))
        .arg(mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("allocator probe exceeded its containment deadline");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn soft_memory_error_is_returned_inside_the_isolated_host() {
    let output = probe("soft-limit");
    assert!(
        output.status.success(),
        "probe failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"soft-memory-limit-enforced\n");
}

#[test]
fn hard_allocator_failure_terminates_only_the_isolated_host() {
    let output = probe("hard-limit");
    assert_eq!(output.status.code(), Some(monty_types::OOM_EXIT_CODE));
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds the memory limit"));
    // The parent remains alive and can start another checked interpreter host.
    assert!(probe("soft-limit").status.success());
}
