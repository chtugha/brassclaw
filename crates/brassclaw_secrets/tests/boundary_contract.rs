#[test]
fn secrets_crate_does_not_depend_on_workflow_runtime_or_observability_crates() {
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
        "brassclaw_host_runtime",
        "brassclaw_mcp",
        "brassclaw_processes",
        "brassclaw_resources",
        "brassclaw_run_state",
        "brassclaw_scripts",
        "brassclaw_wasm",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "brassclaw_secrets must stay a low-level scoped secret service, not depend on {forbidden}"
        );
    }
}
