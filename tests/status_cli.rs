use std::process::Command;

/// Verify that `brassclaw status` exits 0 and prints the version/profile lines
/// from the Reborn status command.
#[test]
fn status_exits_zero_and_prints_version() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let base_dir = tempdir.path();

    let output = Command::new(env!("CARGO_BIN_EXE_brassclaw"))
        .arg("status")
        .env("BRASSCLAW_REBORN_HOME", base_dir)
        .output()
        .expect("run brassclaw status");

    assert!(
        output.status.success(),
        "status command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Status"),
        "status output did not contain header:\n{stdout}"
    );
    assert!(
        stdout.contains("Version"),
        "status output did not contain Version line:\n{stdout}"
    );
}
