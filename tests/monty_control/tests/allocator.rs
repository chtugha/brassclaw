//! Actual child process/allocator evidence, never a simulated memory counter.
use std::process::Command;

#[test]
fn finite_allocator_rejects_overflow_and_still_stops_native_monty_allocation() {
    let worker = env!("CARGO_BIN_EXE_allocator_probe");
    let rejected = Command::new(worker).arg("overflow").output().unwrap();
    assert!(
        rejected.status.success(),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert_eq!(
        String::from_utf8(rejected.stdout).unwrap().trim(),
        "finite budget overflow rejected"
    );
    let stopped = Command::new(worker).arg("memory").output().unwrap();
    assert_eq!(stopped.status.code(), Some(monty_types::OOM_EXIT_CODE));
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("exceeds the memory limit"));
    assert!(stopped.stdout.is_empty());
}
