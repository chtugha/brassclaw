//! Actual child process/allocator evidence, never a simulated memory counter.
use std::process::Command;

#[test]
fn actual_worker_resize_preserves_counters_rejects_unsafe_edits_and_remains_finite() {
    let probe = env!("CARGO_BIN_EXE_allocator_probe");
    let resized = Command::new(probe).arg("resize").output().unwrap();
    assert!(
        resized.status.success(),
        "{}",
        String::from_utf8_lossy(&resized.stderr)
    );
    assert_eq!(
        String::from_utf8(resized.stdout).unwrap().trim(),
        "finite worker limits resized without resetting ownership"
    );
    let stopped = Command::new(probe).arg("resized-memory").output().unwrap();
    assert_eq!(stopped.status.code(), Some(monty_types::OOM_EXIT_CODE));
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("exceeds the memory limit"));
    assert!(stopped.stdout.is_empty());
}

#[test]
fn actual_vm_allocations_keep_ownership_across_scopes_resize_and_cross_thread_free() {
    let result = Command::new(env!("CARGO_BIN_EXE_allocator_probe"))
        .arg("ownership")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        "VM allocation ownership preserved"
    );
}

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
