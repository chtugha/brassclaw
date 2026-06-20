use std::path::PathBuf;

fn runtime_dockerfile() -> String {
    let repo_root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .expect("repo root should be discoverable");
    let path = repo_root.join("Dockerfile");
    std::fs::read_to_string(path).expect("Dockerfile should be readable")
}

#[test]
fn runtime_image_declares_and_prepares_brassclaw_home() {
    let dockerfile = runtime_dockerfile();

    assert!(
        dockerfile.contains("useradd -m -d /home/brassclaw -u 1000 brassclaw"),
        "runtime image must create the brassclaw user with the expected home directory",
    );
    assert!(
        dockerfile.contains("ENV HOME=/home/brassclaw"),
        "runtime image must set HOME to /home/brassclaw for ~/.brassclaw state",
    );
    assert!(
        dockerfile.contains("WORKDIR /home/brassclaw"),
        "runtime image must start in the brassclaw home directory",
    );
    assert!(
        dockerfile.contains("mkdir -p /home/brassclaw/.brassclaw"),
        "runtime image must pre-create ~/.brassclaw before dropping privileges",
    );
}
