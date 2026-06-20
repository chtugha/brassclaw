#[test]
fn approvals_crate_stays_out_of_runtime_and_host_workflow_crates() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {manifest_path:?}: {error}"));
    let dependencies = dependencies_section(&manifest);

    for forbidden in [
        "brassclaw_capabilities",
        "brassclaw_dispatcher",
        "brassclaw_processes",
        "brassclaw_host_runtime",
        "brassclaw_resources",
        "brassclaw_extensions",
        "brassclaw_wasm",
        "brassclaw_scripts",
        "brassclaw_mcp",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "brassclaw_approvals should resolve approval records into leases/audit without depending on {forbidden}"
        );
    }
}

fn dependencies_section(manifest: &str) -> &str {
    manifest
        .split_once("[dependencies]")
        .and_then(|(_, rest)| rest.split_once("[dev-dependencies]").map(|(deps, _)| deps))
        .expect("Cargo.toml must contain [dependencies] before [dev-dependencies]")
}
