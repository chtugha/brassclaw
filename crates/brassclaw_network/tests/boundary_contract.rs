#[test]
fn network_crate_does_not_depend_on_workflow_runtime_secret_or_observability_crates() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {manifest_path:?}: {error}"));

    for forbidden in [
        "brassclaw_authorization",
        "brassclaw_approvals",
        "brassclaw_capabilities",
        "brassclaw_dispatcher",
        "brassclaw_events",
        "brassclaw_extensions",
        "brassclaw_filesystem",
        "brassclaw_host_runtime",
        "brassclaw_mcp",
        "brassclaw_processes",
        "brassclaw_resources",
        "brassclaw_run_state",
        "brassclaw_scripts",
        "brassclaw_secrets",
        "brassclaw_wasm",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "brassclaw_network must stay a low-level scoped network policy service, not depend on {forbidden}"
        );
    }
}
