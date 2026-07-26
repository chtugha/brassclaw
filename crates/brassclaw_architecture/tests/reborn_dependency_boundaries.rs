use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    process::Command,
};

use serde_json::Value;

#[test]
fn reborn_boundary_rules_active_crates_are_workspace_members() {
    // Regression for PR #3212 review: a boundary rule whose crate has a
    // `Cargo.toml` on disk but is missing from `cargo metadata` would
    // previously fail open in `assert_no_normal_workspace_deps`, masking
    // forbidden edges in the unregistered crate. Each active rule must
    // either name a crate that has no directory yet (future-only,
    // tolerated) or a crate that is in the workspace metadata.
    let metadata = cargo_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata must include packages");
    let registered = packages
        .iter()
        .filter_map(|package| package["name"].as_str().map(ToString::to_string))
        .collect::<std::collections::HashSet<_>>();

    let root = workspace_root();
    for rule in boundary_rules() {
        let crate_dir = root.join("crates").join(rule.crate_name);
        let manifest = crate_dir.join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        assert!(
            registered.contains(rule.crate_name),
            "{} has a Cargo.toml at {} but is not registered as a workspace member; \
             add it to the root `Cargo.toml` `workspace.members` so its boundary rule \
             is actually checked",
            rule.crate_name,
            manifest.display()
        );
    }
}

#[test]
fn reborn_virtual_roots_match_storage_placement_contract() {
    let root = workspace_root();
    let path_source = std::fs::read_to_string(root.join("crates/brassclaw_host_api/src/path.rs"))
        .expect("host API path source must be readable");
    let storage_contract =
        std::fs::read_to_string(root.join("docs/reborn/contracts/storage-placement.md"))
            .expect("storage placement contract must be readable");
    let filesystem_contract =
        std::fs::read_to_string(root.join("docs/reborn/contracts/filesystem.md"))
            .expect("filesystem contract must be readable");

    let implemented = extract_virtual_roots_const(&path_source);
    let storage = extract_storage_placement_roots(&storage_contract);
    let filesystem = extract_filesystem_namespace_roots(&filesystem_contract);

    assert_eq!(
        implemented, storage,
        "brassclaw_host_api VIRTUAL_ROOTS must match storage-placement.md canonical roots"
    );
    assert_eq!(
        filesystem, storage,
        "filesystem.md namespace roots must match storage-placement.md canonical roots"
    );
}

#[test]
fn reborn_crate_dependency_boundaries_hold() {
    let metadata = cargo_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata must include packages");
    let dependencies = packages
        .iter()
        .filter_map(package_dependencies)
        .collect::<HashMap<_, _>>();

    assert_no_normal_workspace_deps(
        &dependencies,
        "brassclaw_host_api",
        workspace_brassclaw_crates(&dependencies)
            .into_iter()
            .filter(|name| *name != "brassclaw_host_api")
            .collect::<Vec<_>>(),
    );

    for rule in boundary_rules() {
        assert_no_normal_workspace_deps(&dependencies, rule.crate_name, rule.forbidden);
    }
}

#[test]
fn conversation_trusted_trigger_submitter_stays_conversation_or_composition_owned() {
    let root = workspace_root();
    let mut uses = Vec::new();
    collect_forbidden_string_uses(
        &root.join("crates"),
        "ConversationTrustedTriggerSubmitter",
        &root,
        &mut uses,
    );
    let allowed = BTreeSet::from([
        "crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs",
        "crates/brassclaw_conversations/src/inbound.rs",
    ]);
    let violations = uses
        .into_iter()
        .filter(|path| !allowed.contains(path.as_str()))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "Conversation trusted trigger submission must stay conversations/composition-owned; \
         product adapters and capabilities must use untrusted inbound requests. \
         Unexpected call sites:\n{}",
        violations.join("\n")
    );
}

#[test]
fn conversation_trusted_trigger_submitter_stays_out_of_root_exports() {
    let root = workspace_root();
    let lib_source =
        std::fs::read_to_string(root.join("crates/brassclaw_conversations/src/lib.rs"))
            .expect("conversation lib source must be readable");

    assert!(
        !lib_source.contains("ConversationTrustedTriggerSubmitter"),
        "ConversationTrustedTriggerSubmitter must not be re-exported from brassclaw_conversations; \
         composition should use the trusted_trigger_fire_submitter factory returning the trait object"
    );
}

#[test]
fn conversation_trusted_trigger_classifier_stays_out_of_root_exports() {
    let root = workspace_root();
    let lib_source =
        std::fs::read_to_string(root.join("crates/brassclaw_conversations/src/lib.rs"))
            .expect("conversation lib source must be readable");

    assert!(
        !lib_source.contains("classify_trusted_trigger_inbound_error"),
        "classify_trusted_trigger_inbound_error is submitter policy and must not be re-exported \
         from brassclaw_conversations; composition-owned materialization should classify its own \
         local errors"
    );
    assert!(
        !lib_source.contains("classify_inbound_error"),
        "trusted trigger inbound classification must not be re-exported from \
         brassclaw_conversations; keep it private to conversations-owned submitter policy"
    );
    assert!(
        !lib_source.contains("TrustedTriggerInboundFailureKind"),
        "trusted trigger inbound classification types must not be re-exported from \
         brassclaw_conversations; keep them private to conversations-owned submitter policy"
    );
    assert!(
        !lib_source.contains("pub mod trusted_trigger"),
        "trusted_trigger must stay a private implementation module; root exports should name only \
         the narrow symbols downstream composition needs"
    );
}

#[test]
fn trusted_trigger_submit_request_minting_stays_worker_owned() {
    let root = workspace_root();
    let mut struct_literal_uses = Vec::new();
    collect_forbidden_string_uses(
        &root.join("crates"),
        "TrustedTriggerSubmitRequest {",
        &root,
        &mut struct_literal_uses,
    );
    let allowed_struct_literals = BTreeSet::from([
        "crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs",
        "crates/brassclaw_triggers/src/worker/ports.rs",
    ]);
    let struct_literal_violations = struct_literal_uses
        .into_iter()
        .filter(|path| !allowed_struct_literals.contains(path.as_str()))
        .collect::<Vec<_>>();

    assert!(
        struct_literal_violations.is_empty(),
        "TrustedTriggerSubmitRequest fields must stay private; trusted trigger requests \
         are minted by the trigger worker, not by downstream submitter callers. \
         Unexpected struct literal use:\n{}",
        struct_literal_violations.join("\n")
    );
}

#[test]
fn retired_host_trusted_ingress_token_crate_stays_removed() {
    let root = workspace_root();
    let retired_crate_name = ["brassclaw", "trusted", "ingress"].join("_");
    assert!(
        !root
            .join("crates")
            .join(&retired_crate_name)
            .join("Cargo.toml")
            .exists(),
        "a separate trusted ingress crate must stay absent; trusted trigger \
         submission is sealed by brassclaw_triggers and privately converted inside \
         brassclaw_conversations"
    );

    let metadata = cargo_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata must include packages");
    let package_names = packages
        .iter()
        .filter_map(|package| package["name"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        !package_names.contains(retired_crate_name.as_str()),
        "a separate trusted ingress crate must not be introduced as a workspace crate"
    );

    let dependencies = packages
        .iter()
        .filter_map(package_dependencies)
        .collect::<HashMap<_, _>>();
    let violations = dependencies
        .iter()
        .filter_map(|(crate_name, deps)| {
            deps.iter()
                .any(|dependency| dependency == retired_crate_name.as_str())
                .then_some(crate_name.as_str())
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "a separate trusted ingress crate must not be introduced as a production dependency; \
         trusted trigger submission is now sealed by brassclaw_triggers and privately \
         converted inside brassclaw_conversations. Unexpected dependents:\n{}",
        violations.join("\n")
    );
}

#[test]
fn untrusted_ingress_paths_cannot_submit_host_trusted_inbound() {
    let root = workspace_root();
    let forbidden = [
        ForbiddenUse {
            pattern: "ConversationTrustedTriggerSubmitter",
            reason: "untrusted ingress paths must not construct conversation-owned trusted trigger submitters",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "trusted_trigger_fire_submitter",
            reason: "untrusted ingress paths must not build host-trusted trigger submitters",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "TrustedTriggerSubmitRequest",
            reason: "untrusted ingress paths must not submit host-trusted trigger fires",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "TrustedTriggerFireSubmitter",
            reason: "untrusted ingress paths must not implement host-trusted trigger submission",
            exempt: None,
        },
    ];
    let untrusted_src_roots = [
        "crates/brassclaw_capabilities/src",
        "crates/brassclaw_first_party_extension_ports/src",
        "crates/brassclaw_first_party_extensions/src",
        "crates/brassclaw_host_api/src",
        "crates/brassclaw_host_runtime/src",
        "crates/brassclaw_product_adapters/src",
        "crates/brassclaw_product_adapter_registry/src",
        "crates/brassclaw_product_workflow/src",
        "crates/brassclaw_product_workflow_storage/src",
        "crates/brassclaw_reborn_webui_ingress/src",
        "crates/brassclaw_webui_v2/src",
        "crates/brassclaw_telegram_v2_adapter/src",
    ];

    let mut violations = Vec::new();
    for relative_root in untrusted_src_roots {
        let dir = root.join(relative_root);
        if !dir.exists() {
            continue;
        }
        collect_forbidden_uses(&dir, &root, &forbidden, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "Untrusted ingress, product, and capability paths must not submit or construct host-trusted synthetic inbound requests; \
         those operations belong to the conversations/composition boundary only:\n{}",
        violations.join("\n")
    );
}

#[test]
fn reborn_cli_binary_crate_stays_separate_from_v1_root() {
    let metadata = cargo_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata must include packages");
    let dependencies = packages
        .iter()
        .filter_map(package_dependencies)
        .collect::<HashMap<_, _>>();
    let dependencies_all_kinds = packages
        .iter()
        .filter_map(package_dependencies_all_kinds)
        .collect::<HashMap<_, _>>();

    let root = workspace_root();
    let manifest_path = root.join("crates/brassclaw_reborn_cli/Cargo.toml");
    assert!(
        manifest_path.exists(),
        "Reborn should ship as a separate binary crate at {}",
        manifest_path.display()
    );

    let manifest =
        std::fs::read_to_string(&manifest_path).expect("Reborn CLI manifest must be readable");
    assert!(
        manifest.contains("name = \"brassclaw_reborn_cli\""),
        "Reborn CLI crate package name should be brassclaw_reborn_cli"
    );
    assert!(
        manifest.contains("[[bin]]") && manifest.contains("name = \"brassclaw-reborn\""),
        "Reborn CLI crate must declare the brassclaw-reborn binary explicitly"
    );

    let command_module_paths = [
        "crates/brassclaw_reborn_cli/AGENTS.md",
        "crates/brassclaw_reborn_cli/src/commands/mod.rs",
        "crates/brassclaw_reborn_cli/src/commands/completion.rs",
        "crates/brassclaw_reborn_cli/src/commands/doctor.rs",
        "crates/brassclaw_reborn_cli/src/commands/repl.rs",
        "crates/brassclaw_reborn_cli/src/commands/run.rs",
        "crates/brassclaw_reborn_cli/src/commands/serve.rs",
        "crates/brassclaw_reborn_cli/src/context.rs",
    ];
    for path in command_module_paths {
        assert!(
            root.join(path).exists(),
            "Reborn CLI commands should use an agent-friendly one-command-per-file layout; missing {path}"
        );
    }

    let agent_contract =
        std::fs::read_to_string(root.join("crates/brassclaw_reborn_cli/AGENTS.md"))
            .expect("Reborn CLI crate-local AGENTS.md must be readable");
    for required_phrase in [
        "one command per file",
        "RebornCliContext",
        "no v1 runtime imports",
    ] {
        assert!(
            agent_contract.contains(required_phrase),
            "Reborn CLI AGENTS.md should document `{required_phrase}` for future command agents"
        );
    }

    assert_workspace_deps_exactly(
        &dependencies,
        "brassclaw_reborn_cli",
        [
            "brassclaw_reborn_composition",
            "brassclaw_reborn_config",
            "brassclaw_reborn_traces",
            "brassclaw_reborn_webui_ingress",
            "brassclaw_embedded_postgres",
            "brassclaw_pg",
        ],
        "brassclaw_reborn_cli should enter Reborn through brassclaw_reborn_composition (assembled-runtime and provider-admin facade), brassclaw_reborn_config (boot-config contract), brassclaw_reborn_traces (contributor-side TraceCommons client extracted from the legacy monolith), and brassclaw_reborn_webui_ingress (host-owned WebUI serve lifecycle) only, plus brassclaw_embedded_postgres and brassclaw_pg for the CLI's embedded-Postgres boot path. Adding any other workspace crate here re-opens speculative public API access to internal Reborn types.",
    );
    assert_workspace_deps_exactly(
        &dependencies_all_kinds,
        "brassclaw_reborn_config",
        [],
        "brassclaw_reborn_config must remain a standalone boot contract crate with no BrassClaw workspace dependencies of any dependency kind",
    );

    let runtime_dir = root.join("crates/brassclaw_reborn_cli/src/runtime");
    let mut cli_runtime_source = String::new();
    collect_runtime_rs(&runtime_dir, &mut cli_runtime_source);
    assert!(
        cli_runtime_source.contains("build_reborn_runtime"),
        "Reborn CLI should enter the assembled runtime through brassclaw_reborn_composition::build_reborn_runtime"
    );
    for forbidden in [
        "use brassclaw_host_runtime::",
        "use brassclaw_reborn::",
        "use brassclaw_threads::",
        "use brassclaw_turns::",
        "HostRuntimeServices",
        "build_default_planned_runtime",
    ] {
        assert!(
            !cli_runtime_source.contains(forbidden),
            "Reborn CLI runtime/ must not wire lower-level Reborn runtime pieces directly via `{forbidden}`; keep REPL as a UX shell over brassclaw_reborn_composition."
        );
    }
}

#[test]
fn reborn_host_runtime_services_do_not_expose_lower_substrate_handles() {
    let root = workspace_root();
    let lib = std::fs::read_to_string(root.join("crates/brassclaw_host_runtime/src/lib.rs"))
        .expect("host runtime lib.rs must be readable");
    let services =
        std::fs::read_to_string(root.join("crates/brassclaw_host_runtime/src/services.rs"))
            .expect("host runtime services.rs must be readable");
    let obligations =
        std::fs::read_to_string(root.join("crates/brassclaw_host_runtime/src/obligations.rs"))
            .expect("host runtime obligations.rs must be readable");
    let host_runtime_contract =
        std::fs::read_to_string(root.join("docs/reborn/contracts/host-runtime.md"))
            .expect("host runtime contract must be readable");
    let mcp = std::fs::read_to_string(root.join("crates/brassclaw_mcp/src/lib.rs"))
        .expect("MCP runtime lib.rs must be readable");
    let mcp_manifest = std::fs::read_to_string(root.join("crates/brassclaw_mcp/Cargo.toml"))
        .expect("MCP runtime Cargo.toml must be readable");

    let forbidden_lib_exports = [
        "RuntimeDispatchProcessExecutor",
        "ScriptRuntimeAdapter",
        "McpRuntimeAdapter",
        "WasmRuntimeAdapter",
    ];
    for export in forbidden_lib_exports {
        assert!(
            !lib.contains(export),
            "brassclaw_host_runtime must not re-export lower substrate handle `{export}`; upper Reborn code should enter through HostRuntimeServices::host_runtime / Arc<dyn HostRuntime>"
        );
    }

    let obligations_pub_use = extract_pub_use_block(&lib, "pub use obligations::{");
    let forbidden_obligation_exports = [
        "NetworkObligationPolicyStore",
        "RuntimeSecretInjectionStore",
        "RuntimeSecretInjectionStoreError",
    ];
    for export in forbidden_obligation_exports {
        assert!(
            !obligations_pub_use.contains(export),
            "brassclaw_host_runtime must not re-export lower substrate handoff store `{export}`; upper Reborn code should enter through HostRuntimeServices::host_runtime / Arc<dyn HostRuntime>"
        );
    }

    let forbidden_lib_accessors = [
        "pub use obligations::NetworkObligationPolicyStore",
        "pub use obligations::RuntimeSecretInjectionStore",
        "pub use obligations::RuntimeSecretInjectionStoreError",
        "pub use obligations::*",
        "pub fn with_secret_injection_store(",
        "pub fn with_network_policy_store(",
        "pub fn network(&self) -> &N",
        "pub fn secrets(&self) -> &S",
    ];
    for pattern in forbidden_lib_accessors {
        assert!(
            !lib.contains(pattern),
            "HostHttpEgressService must not expose lower substrate escape hatch `{pattern}`; keep raw network/secret/policy handoff wiring private to host-runtime composition"
        );
    }

    let forbidden_public_services = [
        "pub fn registry(",
        "pub fn filesystem(",
        "pub fn governor(",
        "pub fn authorizer(",
        "pub fn process_services(",
        "pub fn process_host(",
        "pub fn with_wasm_runtime(",
        "pub fn runtime_dispatcher(",
        "pub fn runtime_dispatcher_arc(",
        "pub fn capability_host",
        "pub fn secret_injection_store(",
        "pub fn network_policy_store(",
        "pub fn with_host_http_egress<N, SecretBackend>",
        "pub struct RuntimeDispatchProcessExecutor",
        "pub struct ScriptRuntimeAdapter",
        "pub struct McpRuntimeAdapter",
        "pub struct WasmRuntimeAdapter",
    ];
    for pattern in forbidden_public_services {
        assert!(
            !services.contains(pattern),
            "HostRuntimeServices must not expose lower substrate escape hatch `{pattern}`; keep dispatcher/capability/process handles private to the host-runtime crate"
        );
    }

    let forbidden_obligation_accessors = [
        "pub struct RuntimeSecretInjectionStore",
        "pub enum RuntimeSecretInjectionStoreError",
        "pub struct NetworkObligationPolicyStore",
        "pub fn insert(",
        "pub fn take(",
        "pub fn discard_for_capability(",
        "pub fn with_handoff_stores(",
        "pub fn with_network_policy_store(",
        "pub fn with_secret_injection_store(",
        "pub fn network_policy_store(&self)",
        "pub fn secret_injection_store(&self)",
        "pub fn staged_network_policy_present_for_diagnostics(",
        "pub fn staged_secret_present_for_diagnostics(",
    ];
    for pattern in forbidden_obligation_accessors {
        assert!(
            !obligations.contains(pattern),
            "BuiltinObligationServices and lower handoff stores must not expose lower substrate escape hatch `{pattern}`; keep secret/network handoff stores private to host-runtime composition"
        );
    }

    for required_phrase in [
        "try_with_host_http_egress",
        "low-level host-runtime/test harness escape hatches",
        "upper Reborn crates must not use them",
    ] {
        assert!(
            host_runtime_contract.contains(required_phrase),
            "host-runtime contract should document `{required_phrase}` so raw handoff store seams are not mistaken for upper Reborn APIs"
        );
    }

    let forbidden_script_lane_surface = [
        "RuntimeAdapter",
        "pub struct ScriptRuntimeAdapter",
        "pub fn script_error_kind",
    ];
    let script_runtime_module =
        root.join("crates/brassclaw_host_runtime/src/services/script_runtime.rs");
    if script_runtime_module.exists() {
        let scripts = std::fs::read_to_string(&script_runtime_module)
            .expect("host runtime script_runtime.rs must be readable when present");
        for pattern in forbidden_script_lane_surface {
            assert!(
                !scripts.contains(pattern),
                "brassclaw_host_runtime::script_runtime must not expose host-runtime dispatcher composition surface `{pattern}`; compose script dispatch adapters inside brassclaw_host_runtime"
            );
        }
        assert!(
            !scripts.contains("brassclaw_dispatcher"),
            "brassclaw_host_runtime::script_runtime must not import brassclaw_dispatcher; script dispatcher adapters are host-runtime-private composition owned by the surrounding services layer"
        );
    }

    let forbidden_mcp_lane_surface = [
        "RuntimeAdapter",
        "pub struct McpRuntimeAdapter",
        "pub fn mcp_error_kind",
    ];
    for pattern in forbidden_mcp_lane_surface {
        assert!(
            !mcp.contains(pattern),
            "brassclaw_mcp must not expose host-runtime dispatcher composition surface `{pattern}`; compose MCP dispatch adapters inside brassclaw_host_runtime"
        );
    }
    assert!(
        !mcp_manifest.contains("brassclaw_dispatcher"),
        "brassclaw_mcp must not depend on brassclaw_dispatcher; MCP dispatcher adapters are host-runtime-private composition"
    );
}

fn extract_pub_use_block<'a>(contents: &'a str, start_marker: &str) -> &'a str {
    let Some(start) = contents.find(start_marker) else {
        return "";
    };
    let after_start = &contents[start..];
    let Some(end) = after_start.find("};") else {
        return after_start;
    };
    &after_start[..end]
}

#[test]
fn reborn_turns_public_surface_keeps_runner_api_explicit() {
    let root = workspace_root();
    let lib = std::fs::read_to_string(root.join("crates/brassclaw_turns/src/lib.rs"))
        .expect("turns lib.rs must be readable");

    let forbidden_public_exports = [
        "pub use runner::",
        "pub use crate::runner::",
        "pub use self::runner::",
    ];
    for pattern in forbidden_public_exports {
        assert!(
            !lib.contains(pattern),
            "brassclaw_turns public prelude must not re-export trusted runner transition API `{pattern}`; adapters must import brassclaw_turns::runner explicitly"
        );
    }
}

#[test]
fn reborn_loop_support_llm_wiring_stays_out_of_root_src() {
    let root = workspace_root();
    // Phase 6 removed the legacy v1 `src/` tree. The only permitted
    // survivor is `src/main.rs` — a thin compat shim that delegates to
    // `brassclaw_reborn_cli` and exists solely so legacy E2E fixtures
    // can still invoke `--no-onboard` / `--cli-only` / `--no-db` /
    // `--auto-approve`. Any other file under `src/` re-opens v1-side
    // ownership of Reborn loop wiring and is forbidden.
    let src_dir = root.join("src");
    if src_dir.exists() {
        let mut entries = std::fs::read_dir(&src_dir)
            .unwrap_or_else(|_| panic!("failed to read {}", src_dir.display()));
        while let Some(Ok(entry)) = entries.next() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            assert!(
                name == "main.rs",
                "Phase 6 removed the v1 root `src/` tree; only `src/main.rs` (thin compat shim) is permitted, found `{}`",
                name
            );
        }
    }
    assert!(
        !root.join("src/reborn_loop_support.rs").exists(),
        "Reborn loop LLM wiring must not live under root src/"
    );

    let reborn_gateway = root.join("crates/brassclaw_reborn/src/model_gateway.rs");
    assert!(
        reborn_gateway.exists(),
        "expected Reborn LLM gateway wiring at {}",
        reborn_gateway.display()
    );
    let reborn_gateway_source = std::fs::read_to_string(&reborn_gateway)
        .expect("Reborn model gateway source must be readable");
    assert!(
        reborn_gateway_source.contains("LlmProviderModelGateway"),
        "Reborn LLM gateway wiring should expose LlmProviderModelGateway from crates/brassclaw_reborn"
    );

    let reborn_manifest = std::fs::read_to_string(root.join("crates/brassclaw_reborn/Cargo.toml"))
        .expect("Reborn manifest must be readable");
    assert!(
        reborn_manifest.contains("optional = true")
            && reborn_manifest.contains("default-features = false")
            && reborn_manifest.contains("root-llm-provider"),
        "brassclaw_reborn may reuse root LLM code only behind an explicit feature, without enabling the root app's default postgres/tui feature set"
    );

    // The composition root — the only crate that should pull `brassclaw_reborn`
    // (and through it `brassclaw_llm`) for the assembled runtime — must mirror
    // the same feature-gated discipline. Both `brassclaw_reborn` (transitive)
    // and `brassclaw_llm` (direct) live behind a `root-llm-provider` feature
    // on the composition crate, so a default build of composition stays
    // substrate-only.
    let composition_manifest =
        std::fs::read_to_string(root.join("crates/brassclaw_reborn_composition/Cargo.toml"))
            .expect("Reborn composition manifest must be readable");
    assert!(
        composition_manifest.contains("root-llm-provider")
            && composition_manifest.contains("brassclaw_llm")
            && composition_manifest.contains("optional = true")
            && composition_manifest.contains("default-features = false"),
        "brassclaw_reborn_composition must gate `brassclaw_llm` behind the same `root-llm-provider` feature with `optional = true, default-features = false`"
    );
}

/// Lock the narrowed `brassclaw_reborn` public surface in place.
///
/// `brassclaw_reborn` previously exposed ~25 types as a wall of `pub use`
/// re-exports (capability resolvers, surface profile filters, milestone
/// scope/sink, model route policies, planned-driver factory helpers, the
/// loop-driver-host factory, etc.). Internal-trace audits found that **no
/// crate outside the reborn family ever named any of those items** and that
/// composition does not need them either — it imports via submodule paths
/// (`brassclaw_reborn::driver_registry::DriverRegistry`, etc.). The wall was
/// pure speculative public API.
///
/// This test pins the cleanup: `crates/brassclaw_reborn/src/lib.rs` must be a
/// directory of `pub mod` declarations and nothing else. A future contributor
/// who tries to re-add the convenience `pub use` block fails this test
/// alongside the boundary rule that forbids any non-composition crate from
/// taking a normal cargo dep on `brassclaw_reborn`.
#[test]
fn reborn_internal_crate_keeps_directory_of_modules_lib_rs() {
    let root = workspace_root();
    let lib = std::fs::read_to_string(root.join("crates/brassclaw_reborn/src/lib.rs"))
        .expect("brassclaw_reborn lib.rs must be readable");

    // The forbidden re-export prefixes correspond to the original noisy
    // wall. Anyone wanting these items must reach them through a `pub mod`
    // path or (preferably) consume them through `brassclaw_reborn_composition`.
    let forbidden_reexports = [
        "pub use brassclaw_loop_support::",
        "pub use loop_driver_host::",
        "pub use milestone_events::",
        "pub use model_gateway::",
        "pub use model_routes::",
        "pub use planned_driver::",
        "pub use planned_driver_factory::",
        "pub use text_loop_driver::",
        "pub use app_loop_family::",
    ];
    for forbidden in forbidden_reexports {
        assert!(
            !lib.contains(forbidden),
            "brassclaw_reborn/src/lib.rs must not re-export internal items via `{forbidden}`. \
             Reach them through the `pub mod` path or through brassclaw_reborn_composition. \
             See `reborn_internal_crate_keeps_directory_of_modules_lib_rs` for context."
        );
    }

    // The composition root is the sanctioned consumer of `brassclaw_reborn`'s
    // module paths. Confirm the run-state assembly is wired there (it would
    // otherwise have to live in the CLI or root app, which the dep rules
    // forbid).
    let composition_runtime = root.join("crates/brassclaw_reborn_composition/src/runtime.rs");
    let composition_local_dev_runtime =
        root.join("crates/brassclaw_reborn_composition/src/runtime/local_dev.rs");
    assert!(
        composition_runtime.exists(),
        "expected Reborn runtime assembly at {}",
        composition_runtime.display()
    );
    assert!(
        composition_local_dev_runtime.exists(),
        "expected local-dev runtime assembly at {}",
        composition_local_dev_runtime.display()
    );
    let composition_runtime_source = std::fs::read_to_string(&composition_runtime)
        .expect("composition runtime.rs must be readable");
    let composition_runtime_sources = format!(
        "{}\n{}",
        composition_runtime_source,
        std::fs::read_to_string(&composition_local_dev_runtime)
            .expect("composition runtime/local_dev.rs must be readable")
    );
    for required in [
        "pub async fn build_reborn_runtime",
        "pub struct RebornRuntime",
        "use brassclaw_reborn::runtime::",
        "build_default_planned_runtime",
        "DefaultPlannedRuntimeParts",
    ] {
        assert!(
            composition_runtime_source.contains(required),
            "composition runtime.rs missing `{required}` -- the runtime assembly slice \
             must live in `brassclaw_reborn_composition` so the CLI and other \
             ingress points can avoid importing `brassclaw_reborn` directly."
        );
    }
    assert!(
        composition_runtime_sources.contains("use brassclaw_loop_support::")
            && composition_runtime_sources.contains("LoopCapabilityPortFactory"),
        "composition runtime module set missing loop-support capability factory wiring -- \
         the host adapter assembly may live in a runtime submodule, but it must stay inside \
         `brassclaw_reborn_composition` rather than the CLI or other ingress points."
    );
}

/// Lock the boot-config TOML + provider-catalog layering for the
/// standalone `brassclaw-reborn` binary.
///
/// After Phase 8 (file-based config removal):
///
/// 1. `brassclaw_reborn_config` continues to expose the boot-time parser
///    (`RebornConfigFile`) and `RebornHome::path()` so callers can construct
///    the boot-TOML path (`home.path().join("config.toml")`). The path
///    accessor helpers (`config_file_path` / `providers_file_path`) were
///    removed; callers inline the well-known filenames.
///
/// 2. The boot TOML at `config.toml` is still read at startup. Provider
///    definitions are now DB-backed (`brassclaw_llm_providers` table).
///    For migration compat, `providers.json` may still exist in the home
///    directory; it is read by the `migrate-from-libsql` feature only.
///
/// 3. `RebornConfigFile` rejects inline secret material at parse time.
///    The unit test in `secrets_guard` covers the patterns; this
///    boundary test asserts that the rejection path is *wired through*
///    `RebornConfigFile::validate` (file-level grep).
#[test]
fn reborn_boot_config_file_layout_is_pinned() {
    let root = workspace_root();

    let config_lib =
        std::fs::read_to_string(root.join("crates/brassclaw_reborn_config/src/lib.rs"))
            .expect("reborn config lib.rs must be readable");
    for required_export in [
        "pub use config_file::",
        "RebornConfigFile",
        "REBORN_CONFIG_API_VERSION",
        "InlineSecretError",
    ] {
        assert!(
            config_lib.contains(required_export),
            "brassclaw_reborn_config/src/lib.rs must export `{required_export}`; \
             see reborn_boot_config_file_layout_is_pinned for context"
        );
    }

    let home_src = std::fs::read_to_string(root.join("crates/brassclaw_reborn_config/src/home.rs"))
        .expect("reborn config home.rs must be readable");
    // Phase 8: config_file_path / providers_file_path helpers were removed.
    // Callers now use home.path().join("config.toml") directly.
    // We still pin that `path()` is exposed so callers can construct these paths.
    assert!(
        home_src.contains("pub fn path"),
        "RebornHome must expose `pub fn path` so callers can construct boot-file paths; \
         see reborn_boot_config_file_layout_is_pinned"
    );
    // The boot TOML file name must remain `config.toml` for operator muscle memory.
    assert!(
        home_src.contains("\"config.toml\"") || !home_src.contains("config_file_path"),
        "boot config file name must be `config.toml`; \
         if the accessor was removed callers must inline this exact name"
    );

    // The boot TOML parser must wire the inline-secret guard. A
    // regression that bypasses it (e.g. a future contributor adds a
    // new section and forgets to call `reject_inline_secret`) would
    // silently allow pasted credentials through.
    let config_file_src =
        std::fs::read_to_string(root.join("crates/brassclaw_reborn_config/src/config_file.rs"))
            .expect("reborn config_file.rs must be readable");
    assert!(
        config_file_src.contains("reject_inline_secret"),
        "RebornConfigFile::validate must call `reject_inline_secret` on operator-pasteable \
         fields. See `docs/reborn/contracts/secrets.md` and epic #3036's `Pitfalls & \
         Landmines` section: \"Do not bake secret material into blueprints/config.\""
    );

    // Provider-catalog load-from-path must be reachable from
    // composition without forcing `brassclaw_reborn_config` to depend
    // on `brassclaw_llm` (which would violate _config's standalone
    // boundary). The composition crate is the legitimate consumer.
    let llm_catalog = root.join("crates/brassclaw_reborn_composition/src/llm_catalog.rs");
    assert!(
        llm_catalog.exists(),
        "composition must expose a catalog resolver at {} so the CLI can stitch \
         RebornConfigFile + providers.json into a RebornLlmConfig without itself \
         depending on brassclaw_llm",
        llm_catalog.display()
    );
    let llm_catalog_src = std::fs::read_to_string(&llm_catalog).expect("llm_catalog readable");
    for required in [
        "pub fn resolve_llm_selection_against_catalog",
        "pub fn resolve_against_registry",
    ] {
        assert!(
            llm_catalog_src.contains(required),
            "composition llm_catalog must expose `{required}` so the resolver path is \
             stable; see reborn_boot_config_file_layout_is_pinned"
        );
    }

    // `brassclaw_llm` must expose the path-overridable loader so the
    // catalog file location is selectable per-deployment.
    let llm_registry = std::fs::read_to_string(root.join("crates/brassclaw_llm/src/registry.rs"))
        .expect("brassclaw_llm registry.rs must be readable");
    assert!(
        llm_registry.contains("pub fn load_from_path"),
        "brassclaw_llm::ProviderRegistry must expose `load_from_path` so callers can \
         override the user-overlay catalog path"
    );
}

#[test]
fn reborn_turns_public_surface_uses_turn_ids_not_runtime_or_process_ids() {
    let root = workspace_root();
    let turns_src = root.join("crates/brassclaw_turns/src");
    let mut violations = Vec::new();
    collect_forbidden_turns_identifier_uses(&turns_src, &root, &mut violations);

    assert!(
        violations.is_empty(),
        "brassclaw_turns public API must use TurnId/TurnRunId instead of lower runtime/process identifiers:\n{}",
        violations.join("\n")
    );
}

#[test]
fn phase_4_deleted_wasm_and_script_paths() {
    let workspace = workspace_root();

    let deleted_wasm_crates = [
        "crates/brassclaw_wasm",
        "crates/brassclaw_wasm_sandbox_core",
        "crates/brassclaw_wasm_limiter",
        "crates/brassclaw_wasm_product_adapters",
        "crates/brassclaw_scripts",
    ];

    let missing: Vec<&str> = deleted_wasm_crates
        .iter()
        .filter(|relative| workspace.join(relative).join("Cargo.toml").exists())
        .copied()
        .collect();

    assert!(
        missing.is_empty(),
        "Phase 4 of the v1-removal plan deleted the WASM sandbox crates (wasm, wasm_sandbox_core, \
         wasm_limiter, wasm_product_adapters) and the bespoke script runtime crate (scripts). \
         Each deleted crate's directory must stay absent until a new boundary rule reintroduces it. \
         Reappeared crates: {:?}",
        missing
    );

    let script_runtime_module =
        workspace.join("crates/brassclaw_host_runtime/src/services/script_runtime.rs");
    assert!(
        !script_runtime_module.exists(),
        "Phase 4 also deleted `brassclaw_host_runtime::services::script_runtime`. \
         Subprocess-with-docker isolation moved to `brassclaw_process_sandbox::image::validate_reference`; \
         the bespoke script module must not reappear unless a new boundary rule reintroduces it."
    );

    let forbidden_script_patterns: &[(&str, &str)] = &[
        (
            "RuntimeKind::Script",
            "RuntimeKind::Script was removed in Phase 4; use RuntimeKind::Mcp instead",
        ),
        (
            "ExtensionRuntime::Script",
            "ExtensionRuntime::Script was removed in Phase 4; use ExtensionRuntime::Mcp instead",
        ),
        (
            "ExtensionRuntimeV2::Script",
            "ExtensionRuntimeV2::Script was removed in Phase 4; use ExtensionRuntimeV2::Mcp instead",
        ),
        (
            "DispatchError::Script",
            "DispatchError::Script was removed in Phase 4; use DispatchError::Mcp instead",
        ),
    ];

    let this_test_file =
        workspace.join("crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs");

    let mut script_enum_violations: Vec<String> = Vec::new();
    scan_for_forbidden_patterns(
        &workspace.join("crates"),
        &workspace,
        &this_test_file,
        forbidden_script_patterns,
        &mut script_enum_violations,
    );

    assert!(
        script_enum_violations.is_empty(),
        "Phase 4 removed RuntimeKind::Script, ExtensionRuntime::Script, \
         ExtensionRuntimeV2::Script, and DispatchError::Script from all production source. \
         Note: TrustedRuntimeKindWire::Script is intentionally retained in \
         brassclaw_events/src/runtime_event.rs as a backwards-compatible wire alias \
         that deserialises historical event records as RuntimeKind::Mcp. \
         The following files contain a forbidden pattern that must be removed:\n{}",
        script_enum_violations.join("\n")
    );
}

#[test]
fn reborn_runtime_http_egress_has_single_network_boundary() {
    let forbidden = [
        ForbiddenRuntimeNetworkUse {
            pattern: "reqwest::Client",
            reason: "runtime crates must use brassclaw_network for outbound HTTP transport",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "reqwest::blocking::Client",
            reason: "runtime crates must use brassclaw_network for outbound HTTP transport",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "reqwest::ClientBuilder",
            reason: "runtime crates must use brassclaw_network for outbound HTTP transport",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "ToSocketAddrs",
            reason: "runtime crates must not perform ad-hoc DNS resolution",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: ".to_socket_addrs(",
            reason: "runtime crates must not perform ad-hoc DNS resolution",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "ssrf_safe_client_builder",
            reason: "runtime crates must not reuse V1 WASM SSRF helpers",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "validate_and_resolve_http_target",
            reason: "runtime crates must not reuse V1 WASM SSRF helpers",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "reject_private_ip",
            reason: "runtime crates must not perform ad-hoc SSRF checks",
        },
        ForbiddenRuntimeNetworkUse {
            pattern: "is_private_or_loopback_ip",
            reason: "runtime crates must not perform ad-hoc private-IP checks",
        },
    ];

    let root = workspace_root();
    let runtime_src_roots = [
        "crates/brassclaw_mcp/src",
        "crates/brassclaw_host_runtime/src",
    ];

    let mut violations = Vec::new();
    for relative_root in runtime_src_roots {
        let dir = root.join(relative_root);
        if !dir.exists() {
            continue;
        }
        collect_forbidden_runtime_network_uses(&dir, &root, &forbidden, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "Reborn runtime HTTP must use the shared host egress service and brassclaw_network only:\n{}",
        violations.join("\n")
    );
}

#[test]
fn reborn_product_api_crates_do_not_bind_http_ingress() {
    let forbidden = [
        ForbiddenUse {
            pattern: "tokio::net::TcpListener::bind",
            reason: "Reborn product/API crates must expose route descriptors, not bind listeners",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "std::net::TcpListener::bind",
            reason: "Reborn product/API crates must expose route descriptors, not bind listeners",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "TcpListener::bind",
            reason: "Reborn product/API crates must expose route descriptors, not bind listeners",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "axum::serve",
            reason: "Reborn product/API crates must not own server lifecycle",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "hyper::Server",
            reason: "Reborn product/API crates must not own server lifecycle",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "Server::bind",
            reason: "Reborn product/API crates must not own server lifecycle",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "axum_server::bind",
            reason: "Reborn product/API crates must not own server lifecycle",
            exempt: None,
        },
    ];

    let root = workspace_root();
    let reborn_product_api_src_roots = [
        "crates/brassclaw_reborn/src",
        "crates/brassclaw_reborn_cli/src",
        "crates/brassclaw_reborn_composition/src",
        "crates/brassclaw_reborn_config/src",
        "crates/brassclaw_reborn_event_store/src",
        "crates/brassclaw_reborn_api/src",
        "crates/brassclaw_product_adapters/src",
        "crates/brassclaw_product_adapter_registry/src",
        "crates/brassclaw_product_workflow/src",
        "crates/brassclaw_telegram_v2_adapter/src",
        "crates/brassclaw_outbound/src",
        "crates/brassclaw_conversations/src",
        "crates/brassclaw_turns/src",
        "crates/brassclaw_threads/src",
        "crates/brassclaw_loop_support/src",
        // WebChat v2 route surface: a Product/API crate that exposes
        // axum handler functions and `IngressRouteDescriptor`s but must
        // never bind sockets or call `axum::serve` itself — that is
        // host composition's job. Without this entry the contract fails
        // open for the new route crate.
        "crates/brassclaw_webui_v2/src",
    ];

    let mut violations = Vec::new();
    for relative_root in reborn_product_api_src_roots {
        let dir = root.join(relative_root);
        if !dir.exists() {
            continue;
        }
        collect_forbidden_uses(&dir, &root, &forbidden, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "Reborn HTTP ingress must be host-owned; product/API crates may expose descriptors or route fragments but must not bind/serve listeners:\n{}",
        violations.join("\n")
    );
}

#[test]
fn reborn_product_auth_contract_stays_reborn_native() {
    let forbidden = [
        ForbiddenUse {
            pattern: "brassclaw::",
            reason: "Reborn product auth must not depend on the v1 root crate",
            exempt: Some(is_reborn_tracing_target_line),
        },
        ForbiddenUse {
            pattern: "src/extensions",
            reason: "v1 extension paths are inventory only, not Reborn auth implementation",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "src/channels/web",
            reason: "v1 web routes are inventory only, not Reborn auth implementation",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "ExtensionManager",
            reason: "Reborn product auth must not call through the v1 extension manager",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "PendingOAuth",
            reason: "Reborn product auth must not reuse v1 pending OAuth maps",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "PendingGate",
            reason: "Reborn product auth must not reuse v1 pending gate maps",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "SecretsStore",
            reason: "Reborn product auth must use opaque handles, not raw v1 secrets storage",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "get_decrypted",
            reason: "Reborn product auth must not retrieve raw secret material directly",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "auth-token",
            reason: "Reborn manual-token setup must not fall back to v1 chat token route names",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "auth_token",
            reason: "Reborn manual-token setup must not fall back to v1 chat token command paths",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "IncomingMessage",
            reason: "Reborn product auth must not capture manual tokens through chat transcripts",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "ChatMessage",
            reason: "Reborn product auth must not capture manual tokens through chat transcripts",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "secret_name",
            reason: "Reborn product auth must use scoped credential accounts and opaque handles, not raw v1 secret names",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "SecretName",
            reason: "Reborn product auth must use scoped credential accounts and opaque handles, not raw v1 secret names",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "reqwest",
            reason: "Reborn product auth must not own outbound HTTP transport",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "authorization_code: String",
            reason: "raw OAuth codes must be one-shot non-serializable provider inputs",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "pkce_verifier: String",
            reason: "raw PKCE verifiers must be one-shot non-serializable provider inputs",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "access_token: String",
            reason: "raw provider tokens must not enter product auth contract records",
            exempt: None,
        },
        ForbiddenUse {
            pattern: "refresh_token: String",
            reason: "raw provider tokens must not enter product auth contract records",
            exempt: None,
        },
    ];

    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("crates/brassclaw_auth/Cargo.toml"))
        .expect("brassclaw_auth manifest must be readable");
    assert!(
        !manifest.contains("reqwest"),
        "brassclaw_auth must not depend on reqwest directly; provider transport belongs behind Reborn-native composition"
    );

    let auth_src = root.join("crates/brassclaw_auth/src");
    assert!(
        auth_src.exists(),
        "Reborn product auth contract crate must have a src directory at {}",
        auth_src.display()
    );

    let mut violations = Vec::new();
    collect_forbidden_uses(&auth_src, &root, &forbidden, &mut violations);
    collect_forbidden_reborn_auth_file_uses(
        &root.join("crates/brassclaw_reborn_composition/src/auth.rs"),
        &root,
        &forbidden,
        &mut violations,
    );
    collect_forbidden_reborn_auth_path_uses(
        &root.join("crates/brassclaw_reborn_composition/src/product_auth_serve"),
        &root.join("crates/brassclaw_reborn_composition/src/product_auth_serve.rs"),
        &root,
        &forbidden,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "Reborn product auth can be behavior-compatible with v1, but implementation and composition code paths must not mingle with v1 routes, v1 extension/secrets managers, raw provider transport, or raw secret records:\n{}",
        violations.join("\n")
    );
}

struct ForbiddenRuntimeNetworkUse {
    pattern: &'static str,
    reason: &'static str,
}

struct ForbiddenUse {
    pattern: &'static str,
    reason: &'static str,
    exempt: Option<fn(&str) -> bool>,
}

fn collect_forbidden_turns_identifier_uses(
    dir: &std::path::Path,
    root: &std::path::Path,
    violations: &mut Vec<String>,
) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
        let path = entry.path();
        if path.is_dir() {
            collect_forbidden_turns_identifier_uses(&path, root, violations);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        for pattern in ["InvocationId", "ProcessId"] {
            if contents.contains(pattern) {
                violations.push(format!(
                    "{} contains forbidden lower identifier `{pattern}`",
                    path.strip_prefix(root).unwrap_or(&path).display()
                ));
            }
        }
    }
}

fn collect_forbidden_string_uses(
    dir: &std::path::Path,
    needle: &str,
    root: &std::path::Path,
    matches: &mut Vec<String>,
) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
        let path = entry.path();
        if path.is_dir() {
            collect_forbidden_string_uses(&path, needle, root, matches);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        if contents.contains(needle) {
            matches.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
}

struct BoundaryRule {
    crate_name: &'static str,
    forbidden: Vec<&'static str>,
}

fn boundary_rules() -> Vec<BoundaryRule> {
    vec![
        BoundaryRule {
            crate_name: "brassclaw_product_workflow",
            forbidden: vec![
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_network",
                "brassclaw_engine",
                "brassclaw_gateway",
            ],
        },
        BoundaryRule {
            // Product auth is a Reborn contract/facade vocabulary. It may
            // describe behavior-compatible v1 inventory, but implementation
            // code must not reach into v1 routes, extension managers, secret
            // stores, runtimes, or channel-specific stacks.
            crate_name: "brassclaw_auth",
            forbidden: vec![
                "brassclaw",
                "brassclaw_approvals",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_event_projections",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_llm",
                "brassclaw_loop_support",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_adapters",
                "brassclaw_product_adapter_registry",
                "brassclaw_product_workflow",
                "brassclaw_reborn",
                "brassclaw_reborn_cli",
                "brassclaw_reborn_composition",
                "brassclaw_reborn_config",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_storage",
                "brassclaw_threads",
                "brassclaw_trust",
                "brassclaw_tui",
                "brassclaw_turns",
            ],
        },
        BoundaryRule {
            // WebChat v2 route surface must only reach into Reborn through
            // the host-facing facade and the ingress vocabulary; anything
            // that lets a handler touch the dispatcher, runtime lane, run
            // state, or a storage backend directly would defeat the
            // single-facade discipline that this crate exists to enforce.
            crate_name: "brassclaw_webui_v2",
            forbidden: vec![
                "brassclaw",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_event_projections",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_llm",
                "brassclaw_loop_support",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                // Single-facade boundary: route handlers consume only the
                // `brassclaw_product_workflow` facade plus the ingress + error
                // vocabulary. Projection types are re-exported through the
                // facade crate so handlers never reach into the adapter
                // surface directly.
                "brassclaw_product_adapters",
                "brassclaw_reborn",
                "brassclaw_reborn_cli",
                "brassclaw_reborn_composition",
                "brassclaw_reborn_config",
                "brassclaw_reborn_event_store",
                "brassclaw_first_party_extensions",
                "brassclaw_first_party_extension_ports",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_storage",
                "brassclaw_threads",
                "brassclaw_trust",
                "brassclaw_tui",
                "brassclaw_turns",
            ],
        },
        BoundaryRule {
            // Registry projects ProductAdapter host-api sections from the single
            // Extension Manifest v2 over extension-owned installation and activation
            // state. Runtime/dispatcher/engine crates would invert ownership, secrets
            // crates could expose raw material instead of opaque handles, and v1
            // WASM/channel crates would bypass the Reborn registry boundary.
            crate_name: "brassclaw_product_adapter_registry",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_workflow",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_threads",
                "brassclaw_tui",
            ],
        },
        BoundaryRule {
            // First-party extensions are userland implementation packages.
            // They may consume scoped storage and pure safety helpers, but
            // must not receive ambient runtime authority or loop-facing
            // runtime handles.
            crate_name: "brassclaw_first_party_extensions",
            forbidden: vec![
                "brassclaw",
                "brassclaw_approvals",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_first_party_extension_ports",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_llm",
                "brassclaw_loop_support",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_adapters",
                "brassclaw_product_workflow",
                "brassclaw_product_adapter_registry",
                "brassclaw_reborn",
                "brassclaw_reborn_composition",
                "brassclaw_reborn_config",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_secrets",
                "brassclaw_threads",
                "brassclaw_tui",
            ],
        },
        BoundaryRule {
            // First-party extension ports are adapter glue above concrete
            // userland implementations. They may depend on loop/turn-facing
            // contracts, but must not reach into host runtime authority or
            // product composition.
            crate_name: "brassclaw_first_party_extension_ports",
            forbidden: vec![
                "brassclaw",
                "brassclaw_approvals",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_llm",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_adapters",
                "brassclaw_product_workflow",
                "brassclaw_product_adapter_registry",
                "brassclaw_reborn",
                "brassclaw_reborn_composition",
                "brassclaw_reborn_config",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_tui",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_reborn_config",
            forbidden: vec![
                "brassclaw",
                "brassclaw_approvals",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_api",
                "brassclaw_host_runtime",
                "brassclaw_llm",
                "brassclaw_loop_support",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_adapters",
                "brassclaw_reborn",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_threads",
                "brassclaw_trust",
                "brassclaw_tui",
                "brassclaw_turns",
            ],
        },
        BoundaryRule {
            // The standalone CLI reaches runtime and provider/admin UX through
            // `brassclaw_reborn_composition` facades. Adding any of the
            // forbidden deps here re-opens "speculative public API" access to
            // internal Reborn types (turn coordinator, session thread service,
            // loop drivers, LLM registry/auth internals, etc.) and
            // re-introduces the narrow-surface regression this rule exists to
            // prevent.
            crate_name: "brassclaw_reborn_cli",
            forbidden: vec![
                "brassclaw",
                "brassclaw_engine",
                "brassclaw_gateway",
                "brassclaw_llm",
                "brassclaw_loop_support",
                "brassclaw_reborn",
                "brassclaw_skills",
                "brassclaw_threads",
                "brassclaw_tui",
                "brassclaw_turns",
            ],
        },
        BoundaryRule {
            // Host-owned WebUI ingress: binds the TCP listener and runs
            // the axum serve loop for the composed v2 Router. Deliberately
            // narrow: it must not pull product/API internals, lower
            // substrate handles, or v1 surface code into the binary path.
            // Reaches Reborn through brassclaw_reborn_composition's facade
            // only (Router + WebuiAuthenticator trait + WebuiServeConfig).
            crate_name: "brassclaw_reborn_webui_ingress",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_llm",
                "brassclaw_loop_support",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_adapters",
                "brassclaw_product_adapter_registry",
                "brassclaw_product_workflow",
                "brassclaw_reborn",
                "brassclaw_reborn_cli",
                "brassclaw_reborn_config",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_threads",
                "brassclaw_trust",
                "brassclaw_tui",
                "brassclaw_turns",
                "brassclaw_webui_v2",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_filesystem",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_memory",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_resources",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_extensions",
                // brassclaw_filesystem is permitted: FilesystemResourceGovernorStore
                // routes the resource-governor snapshot through ScopedFilesystem
                // under the universal-fs-dispatch rework (plan
                // 2026-05-14-universal-fs-dispatch).
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_trust",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_extensions",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_first_party_extensions",
                "brassclaw_first_party_extension_ports",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_events",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            // Product-facing projection reducers consume typed domain events.
            // `brassclaw_turns` is intentionally allowed here for
            // `TurnLifecycleEvent`-derived read models such as pending gates;
            // projection crates must still stay below product/runtime
            // composition and must not import root `src/` or legacy engine
            // pending-gate types.
            crate_name: "brassclaw_event_projections",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_host_runtime",
                "brassclaw_reborn_event_store",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_event_streams",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_processes",
                "brassclaw_product_adapter_registry",
                "brassclaw_product_adapters",
                "brassclaw_product_workflow",
                "brassclaw_product_workflow_storage",
                "brassclaw_reborn_event_store",
                "brassclaw_reborn",
                "brassclaw_reborn_cli",
                "brassclaw_reborn_composition",
                "brassclaw_reborn_config",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_telegram_v2_adapter",
                "brassclaw_threads",
                "brassclaw_trust",
                "brassclaw_tui",
                "brassclaw_webui_v2",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_outbound",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_conversations",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                // brassclaw_filesystem is permitted: FilesystemOutboundStateStore
                // routes outbound persistence through ScopedFilesystem under
                // the universal-fs-dispatch rework (plan
                // 2026-05-14-universal-fs-dispatch).
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_processes",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_tui",
            ],
        },
        BoundaryRule {
            // Trigger core owns source evaluation and trigger-domain state.
            // Durable storage, poller lifecycle, capability registration,
            // product adapters, and outbound delivery are wired by later
            // owners, not by reaching upward from this crate.
            crate_name: "brassclaw_triggers",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_outbound",
                "brassclaw_processes",
                "brassclaw_product_adapter_registry",
                "brassclaw_product_adapters",
                "brassclaw_product_workflow",
                "brassclaw_product_workflow_storage",
                "brassclaw_reborn",
                "brassclaw_reborn_cli",
                "brassclaw_reborn_composition",
                "brassclaw_reborn_config",
                "brassclaw_reborn_event_store",
                "brassclaw_resources",
                "brassclaw_run_state",
                "brassclaw_runtime_policy",
                "brassclaw_safety",
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_threads",
                "brassclaw_trust",
                "brassclaw_tui",
                "brassclaw_webui_v2",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_reborn_event_store",
            // brassclaw_filesystem is permitted: FilesystemEventLog routes the
            // durable log through the universal RootFilesystem dispatch
            // fabric. See `2026-05-14-universal-fs-dispatch.md`.
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_secrets",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_extensions",
                // brassclaw_filesystem is permitted: FilesystemSecretStore /
                // FilesystemCredentialBroker route secret + credential
                // persistence through ScopedFilesystem under the
                // universal-fs-dispatch rework (plan
                // 2026-05-14-universal-fs-dispatch).
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_network",
            forbidden: vec![
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
                "brassclaw_secrets",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_authorization",
            forbidden: vec![
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_run_state",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_events",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_threads",
            forbidden: vec![
                "brassclaw",
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_engine",
                "brassclaw_events",
                "brassclaw_extensions",
                // brassclaw_filesystem is permitted: FilesystemSessionThreadService
                // routes thread/transcript persistence through ScopedFilesystem
                // under the universal-fs-dispatch rework (plan
                // 2026-05-14-universal-fs-dispatch).
                "brassclaw_gateway",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_processes",
                "brassclaw_resources",
                "brassclaw_run_state",
                // brassclaw_safety is permitted: thread/transcript storage
                // validates provider-originated replay metadata before it can
                // be persisted or exposed back to a model-visible context.
                "brassclaw_secrets",
                "brassclaw_skills",
                "brassclaw_tui",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_approvals",
            forbidden: vec![
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_resources",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_processes",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_run_state",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_turns",
            forbidden: vec![
                "brassclaw_approvals",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                // brassclaw_filesystem is permitted: FilesystemTurnStateStore
                // routes turn-coordination persistence through ScopedFilesystem
                // under the universal-fs-dispatch rework (plan
                // 2026-05-14-universal-fs-dispatch).
                "brassclaw_hooks",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_processes",
                "brassclaw_run_state",
                "brassclaw_secrets",
            ],
        },
        // The hooks framework depends on `brassclaw_turns` and host primitives
        // but must not pull in runtime adapters or dispatcher concretions.
        // This keeps the contract surface narrow and prevents the framework
        // from acquiring authority it should not have.
        BoundaryRule {
            crate_name: "brassclaw_hooks",
            forbidden: vec![
                "brassclaw_approvals",
                "brassclaw_authorization",
                "brassclaw_capabilities",
                "brassclaw_dispatcher",
                "brassclaw_extensions",
                "brassclaw_filesystem",
                "brassclaw_host_runtime",
                "brassclaw_mcp",
                "brassclaw_memory",
                "brassclaw_network",
                "brassclaw_processes",
                "brassclaw_reborn",
                "brassclaw_run_state",
                "brassclaw_secrets",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_capabilities",
            forbidden: vec![
                "brassclaw_dispatcher",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
            ],
        },
        BoundaryRule {
            crate_name: "brassclaw_dispatcher",
            forbidden: vec![
                "brassclaw_authorization",
                "brassclaw_approvals",
                "brassclaw_capabilities",
                "brassclaw_host_runtime",
                "brassclaw_secrets",
                "brassclaw_network",
                "brassclaw_mcp",
                "brassclaw_processes",
                "brassclaw_run_state",
            ],
        },
    ]
}

fn cargo_metadata() -> Value {
    let manifest_path = workspace_root().join("Cargo.toml");
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(&manifest_path)
        .output()
        .unwrap_or_else(|error| panic!("failed to run cargo metadata: {error}"));

    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata output must be JSON")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("architecture crate must live under crates/brassclaw_architecture")
        .to_path_buf()
}

fn extract_virtual_roots_const(source: &str) -> BTreeSet<String> {
    let const_body = source
        .split("const VIRTUAL_ROOTS: &[&str] = &[")
        .nth(1)
        .and_then(|tail| tail.split("];").next())
        .expect("VIRTUAL_ROOTS const array must be present");
    extract_quoted_absolute_paths(const_body)
}

fn extract_storage_placement_roots(contract: &str) -> BTreeSet<String> {
    contract
        .lines()
        .filter_map(|line| {
            let root = line
                .strip_prefix("| `")?
                .split('`')
                .next()
                .expect("table cell must close code span");
            let root = if root.starts_with("/engine/") {
                "/engine"
            } else {
                root
            };
            Some(root.to_string())
        })
        .filter(|root| is_canonical_virtual_root(root))
        .collect()
}

fn extract_filesystem_namespace_roots(contract: &str) -> BTreeSet<String> {
    let roots_block = contract
        .split("Frozen V1 canonical virtual roots")
        .nth(1)
        .and_then(|tail| tail.split("Recommended meaning:").next())
        .expect("filesystem.md must list frozen V1 canonical virtual roots");
    roots_block
        .lines()
        .map(str::trim)
        .filter(|line| is_canonical_virtual_root(line))
        .map(ToString::to_string)
        .collect()
}

fn extract_quoted_absolute_paths(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('"')?.split('"').next())
        .filter(|root| is_canonical_virtual_root(root))
        .map(ToString::to_string)
        .collect()
}

fn is_canonical_virtual_root(value: &str) -> bool {
    matches!(
        value,
        "/engine"
            | "/system/settings"
            | "/system/extensions"
            | "/system/skills"
            | "/users"
            | "/projects"
            | "/memory"
            | "/artifacts"
            | "/tmp"
            | "/secrets"
            | "/events"
    )
}

fn package_dependencies(package: &Value) -> Option<(String, Vec<String>)> {
    let name = package["name"].as_str()?.to_string();
    let dependencies = workspace_dependency_names(package)
        .filter(|dependency| is_normal_dependency(dependency))
        .filter_map(|dependency| dependency["name"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    Some((name, dependencies))
}

fn package_dependencies_all_kinds(package: &Value) -> Option<(String, Vec<String>)> {
    let name = package["name"].as_str()?.to_string();
    let dependencies = workspace_dependency_names(package)
        .filter_map(|dependency| dependency["name"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    Some((name, dependencies))
}

fn workspace_dependency_names(package: &Value) -> impl Iterator<Item = &Value> {
    package["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|dependency| {
            dependency["name"]
                .as_str()
                .is_some_and(|name| name == "brassclaw" || name.starts_with("brassclaw_"))
        })
}

fn is_normal_dependency(dependency: &Value) -> bool {
    dependency
        .get("kind")
        .and_then(Value::as_str)
        .is_none_or(|kind| kind == "normal")
}

fn workspace_brassclaw_crates(dependencies: &HashMap<String, Vec<String>>) -> Vec<&str> {
    dependencies
        .keys()
        .filter_map(|name| {
            (name == "brassclaw" || name.starts_with("brassclaw_")).then_some(name.as_str())
        })
        .collect()
}

fn assert_workspace_deps_exactly<'a>(
    dependencies: &HashMap<String, Vec<String>>,
    crate_name: &str,
    expected: impl IntoIterator<Item = &'a str>,
    message: &str,
) {
    let actual = dependencies
        .get(crate_name)
        .unwrap_or_else(|| panic!("{crate_name} must be in cargo metadata"))
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let expected = expected
        .into_iter()
        .map(ToString::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected, "{message}");
}

fn assert_no_normal_workspace_deps<'a>(
    dependencies: &HashMap<String, Vec<String>>,
    crate_name: &str,
    forbidden: impl IntoIterator<Item = &'a str>,
) {
    let Some(actual) = dependencies.get(crate_name) else {
        // The landing plan introduces Reborn crates in grouped PRs. Boundary
        // rules become active as soon as their crate is present in the
        // workspace, while absent future crates are ignored in earlier slices.
        // `reborn_boundary_rules_active_crates_are_workspace_members` covers
        // present-on-disk crates that are missing from `cargo metadata`.
        return;
    };
    for forbidden in forbidden {
        assert!(
            !actual.iter().any(|dependency| dependency == forbidden),
            "{crate_name} must not have a normal dependency on {forbidden}; actual normal brassclaw deps: {actual:?}"
        );
    }
}

/// Recursively concatenate every `.rs` file under `dir` into `out`,
/// descending into subdirectories. Matches the recursion pattern used by
/// `collect_forbidden_*` walkers above so future boundary checks over
/// `runtime/` can reuse the same helper. Used by
/// `reborn_cli_binary_crate_stays_separate_from_v1_root` to scan the
/// entire `runtime/` module tree for forbidden imports.
fn collect_runtime_rs(dir: &std::path::Path, out: &mut String) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|err| {
        panic!(
            "Reborn CLI runtime directory must be readable at {}: {err}",
            dir.display()
        )
    }) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_runtime_rs(&path, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "Reborn CLI runtime file {} unreadable: {err}",
                path.display()
            )
        });
        out.push_str(&content);
        out.push('\n');
    }
}

fn collect_forbidden_runtime_network_uses(
    dir: &std::path::Path,
    root: &std::path::Path,
    forbidden: &[ForbiddenRuntimeNetworkUse],
    violations: &mut Vec<String>,
) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read dir entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            collect_forbidden_runtime_network_uses(&path, root, forbidden, violations);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for (line_number, line) in contents.lines().enumerate() {
            for rule in forbidden {
                if line.contains(rule.pattern) {
                    let relative = path.strip_prefix(root).unwrap_or(&path);
                    violations.push(format!(
                        "{}:{} contains `{}` ({})",
                        relative.display(),
                        line_number + 1,
                        rule.pattern,
                        rule.reason
                    ));
                }
            }
        }
    }
}

fn collect_forbidden_uses(
    dir: &std::path::Path,
    root: &std::path::Path,
    forbidden: &[ForbiddenUse],
    violations: &mut Vec<String>,
) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read dir entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            collect_forbidden_uses(&path, root, forbidden, violations);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for (line_number, line) in contents.lines().enumerate() {
            for rule in forbidden {
                if rule.exempt.is_some_and(|exempt| exempt(line)) {
                    continue;
                }
                if line.contains(rule.pattern) {
                    let relative = path.strip_prefix(root).unwrap_or(&path);
                    violations.push(format!(
                        "{}:{} contains `{}` ({})",
                        relative.display(),
                        line_number + 1,
                        rule.pattern,
                        rule.reason
                    ));
                }
            }
        }
    }
}

fn scan_for_forbidden_patterns(
    dir: &std::path::Path,
    root: &std::path::Path,
    skip_file: &std::path::Path,
    patterns: &[(&str, &str)],
    violations: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            scan_for_forbidden_patterns(&path, root, skip_file, patterns, violations);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        if path == skip_file {
            continue;
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let relative = path.strip_prefix(root).unwrap_or(&path);
        for (line_number, line) in contents.lines().enumerate() {
            for (pattern, reason) in patterns {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{} contains `{}` ({})",
                        relative.display(),
                        line_number + 1,
                        pattern,
                        reason,
                    ));
                }
            }
        }
    }
}

fn collect_forbidden_reborn_auth_path_uses(
    module_dir: &std::path::Path,
    legacy_file: &std::path::Path,
    root: &std::path::Path,
    forbidden: &[ForbiddenUse],
    violations: &mut Vec<String>,
) {
    if module_dir.is_dir() {
        collect_forbidden_uses(module_dir, root, forbidden, violations);
        return;
    }
    collect_forbidden_reborn_auth_file_uses(legacy_file, root, forbidden, violations);
}

fn collect_forbidden_reborn_auth_file_uses(
    path: &std::path::Path,
    root: &std::path::Path,
    forbidden: &[ForbiddenUse],
    violations: &mut Vec<String>,
) {
    let message = format!(
        "failed to read Reborn product-auth boundary file {}",
        path.display()
    );
    let contents = std::fs::read_to_string(path).expect(&message);
    for (line_number, line) in contents.lines().enumerate() {
        for rule in forbidden {
            if rule.exempt.is_some_and(|exempt| exempt(line)) {
                continue;
            }
            if !line.contains(rule.pattern) {
                continue;
            }
            violations.push(format!(
                "{}:{} contains forbidden product-auth implementation pattern `{}`: {}",
                path.strip_prefix(root).unwrap_or(path).display(),
                line_number + 1,
                rule.pattern,
                rule.reason
            ));
        }
    }
}

fn is_reborn_tracing_target_line(line: &str) -> bool {
    line.contains("target: \"brassclaw::reborn::")
        || line.contains("target = \"brassclaw::reborn::")
}

#[test]
fn collect_forbidden_reborn_auth_file_uses_detects_violation() {
    let root = std::env::temp_dir().join(format!(
        "brassclaw-reborn-auth-boundary-test-{}",
        std::process::id()
    ));
    let src = root.join("crates/brassclaw_reborn_composition/src");
    std::fs::create_dir_all(&src).expect("test source directory must be created");
    let auth_rs = src.join("auth.rs");
    std::fs::write(&auth_rs, "fn forbidden() { let _ = \"reqwest\"; }\n")
        .expect("test auth.rs must be written");

    let mut violations = Vec::new();
    collect_forbidden_reborn_auth_file_uses(
        &auth_rs,
        &root,
        &[ForbiddenUse {
            pattern: "reqwest",
            reason: "provider transport must stay outside product auth composition",
            exempt: None,
        }],
        &mut violations,
    );

    std::fs::remove_dir_all(&root).expect("test source directory must be removed");

    assert_eq!(violations.len(), 1);
    assert!(
        violations[0].contains("crates/brassclaw_reborn_composition/src/auth.rs"),
        "violation should report the relative auth.rs path: {:?}",
        violations
    );
    assert!(
        violations[0].contains("provider transport must stay outside product auth composition"),
        "violation should report the forbidden-use reason: {:?}",
        violations
    );
}

#[test]
fn collect_forbidden_reborn_auth_file_uses_allows_reborn_tracing_targets() {
    let root = std::env::temp_dir().join(format!(
        "brassclaw-reborn-auth-boundary-tracing-test-{}",
        std::process::id()
    ));
    let src = root.join("crates/brassclaw_reborn_composition/src");
    std::fs::create_dir_all(&src).expect("test source directory must be created");
    let auth_rs = src.join("auth.rs");
    std::fs::write(
        &auth_rs,
        "fn allowed() { tracing::warn!(target: \"brassclaw::reborn::product_auth::oauth\"); }\n",
    )
    .expect("test auth.rs must be written");

    let mut violations = Vec::new();
    collect_forbidden_reborn_auth_file_uses(
        &auth_rs,
        &root,
        &[ForbiddenUse {
            pattern: "brassclaw::",
            reason: "Reborn product auth must not depend on the v1 root crate",
            exempt: Some(is_reborn_tracing_target_line),
        }],
        &mut violations,
    );

    std::fs::remove_dir_all(&root).expect("test source directory must be removed");

    assert!(
        violations.is_empty(),
        "Reborn tracing targets are log namespaces, not v1 root crate references: {:?}",
        violations
    );
}

#[test]
fn collect_forbidden_uses_allows_reborn_tracing_targets() {
    let root = std::env::temp_dir().join(format!(
        "brassclaw-reborn-auth-boundary-dir-tracing-test-{}",
        std::process::id()
    ));
    let src = root.join("crates/brassclaw_reborn_composition/src/product_auth_serve");
    std::fs::create_dir_all(&src).expect("test source directory must be created");
    let mod_rs = src.join("mod.rs");
    std::fs::write(
        &mod_rs,
        "fn allowed() { tracing::warn!(target: \"brassclaw::reborn::product_auth::oauth\"); }\n",
    )
    .expect("test mod.rs must be written");

    let mut violations = Vec::new();
    collect_forbidden_uses(
        &src,
        &root,
        &[ForbiddenUse {
            pattern: "brassclaw::",
            reason: "Reborn product auth must not depend on the v1 root crate",
            exempt: Some(is_reborn_tracing_target_line),
        }],
        &mut violations,
    );

    std::fs::remove_dir_all(&root).expect("test source directory must be removed");

    assert!(
        violations.is_empty(),
        "Directory scanner should treat Reborn tracing targets as log namespaces: {:?}",
        violations
    );
}

#[test]
fn collect_forbidden_uses_detects_violation() {
    let root = std::env::temp_dir().join(format!(
        "brassclaw-forbidden-use-dir-test-{}",
        std::process::id()
    ));
    let src = root.join("crates/example/src");
    std::fs::create_dir_all(&src).expect("test source directory must be created");
    let mod_rs = src.join("mod.rs");
    std::fs::write(&mod_rs, "fn forbidden() { let _ = \"reqwest\"; }\n")
        .expect("test mod.rs must be written");

    let mut violations = Vec::new();
    collect_forbidden_uses(
        &src,
        &root,
        &[ForbiddenUse {
            pattern: "reqwest",
            reason: "provider transport must stay outside product auth composition",
            exempt: None,
        }],
        &mut violations,
    );

    std::fs::remove_dir_all(&root).expect("test source directory must be removed");

    assert_eq!(violations.len(), 1);
    assert!(
        violations[0].contains("crates/example/src/mod.rs"),
        "violation should report the relative mod.rs path: {:?}",
        violations
    );
    assert!(
        violations[0].contains("provider transport must stay outside product auth composition"),
        "violation should report the forbidden-use reason: {:?}",
        violations
    );
}

/// Phase 9 invariant: no `std::fs::read_to_string` or `File::open` in any
/// non-migration production path.
///
/// After Phase 6 (libSQL removal), all persistent state lives in Postgres.
/// The only legitimate filesystem reads in production code are:
/// - The `migrate-from-libsql` migration module (reads legacy config.toml /
///   providers.json to migrate them into the DB).
/// - The architecture tests themselves (which read source files).
///
/// Any other use of the blocking filesystem-read APIs is either dead code or
/// a regression that re-introduces file-based state. This test enforces the
/// invariant crate-wide so a future contributor who adds a new `File::open`
/// outside the migration gate fails clearly.
#[test]
fn no_direct_fs_reads_outside_migration_path() {
    let root = workspace_root();
    let crates_dir = root.join("crates");
    let this_test_file =
        root.join("crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs");
    let composition_test_file =
        root.join("crates/brassclaw_architecture/tests/reborn_composition_boundaries.rs");

    // Patterns that signal a blocking filesystem read in production code.
    let forbidden_patterns: &[(&str, &str)] = &[
        (
            "std::fs::read_to_string",
            "production code must not read files directly; all state lives in Postgres",
        ),
        (
            "fs::read_to_string",
            "production code must not read files directly; all state lives in Postgres",
        ),
        (
            "File::open",
            "production code must not open files directly; all state lives in Postgres",
        ),
    ];

    // Modules that legitimately read files from disk. The Phase 9 invariant
    // ("all state lives in Postgres") is about persisted application state —
    // not about the categories below, which read files the operator or user
    // points the binary at, or which manage the Postgres instance itself.
    //
    // Each entry is `(path_suffix, justification)`. A file is exempt if its
    // path ends with the suffix.
    let allowlist: &[(&str, &str)] = &[
        (
            "brassclaw_reborn_composition/build.rs",
            "build script reads bundled skill/prompt markdown at compile time",
        ),
        (
            "brassclaw_reborn_composition/src/factory.rs",
            "reads the local-dev secrets master key file (secrets ceremony)",
        ),
        (
            "brassclaw_reborn_composition/src/default_system_prompt.rs",
            "reads the bundled default system prompt template at startup",
        ),
        (
            "brassclaw_reborn_composition/src/provider_repo.rs",
            "reads provider config files from the operator-managed provider repo",
        ),
        (
            "brassclaw_reborn_composition/src/secrets_master.rs",
            "secrets master key ceremony — reads passphrase/key files per BRASSCLAW_SECRETS_PASSPHRASE_FILE",
        ),
        (
            "brassclaw_reborn_traces/src/contribution.rs",
            "reads local trace submission records and trace files for TraceCommons contribution",
        ),
        (
            "brassclaw_host_runtime/src/process_output.rs",
            "host runtime reads host process output files — that is its purpose",
        ),
        (
            "brassclaw_embedded_postgres/src/health.rs",
            "manages the embedded Postgres instance — reads postgres data dir for health checks",
        ),
        (
            "brassclaw_embedded_postgres/src/initdb.rs",
            "manages the embedded Postgres instance — reads postgres config during initdb",
        ),
        (
            "brassclaw_reborn_cli/src/commands/traces/mod.rs",
            "CLI traces subcommand — reads trace files the user points at for import/export",
        ),
        (
            "brassclaw_reborn_cli/src/commands/secrets.rs",
            "CLI secrets subcommand — reads key files the operator points at for import/export",
        ),
        (
            "brassclaw_filesystem/src/local.rs",
            "filesystem capability crate — reading files the user explicitly requests is its purpose",
        ),
        (
            "brassclaw_llm/src/session.rs",
            "LLM provider session — loads OAuth session token from disk",
        ),
        (
            "brassclaw_llm/src/gemini_oauth.rs",
            "LLM provider OAuth — loads Gemini OAuth credentials from disk",
        ),
        (
            "brassclaw_llm/src/registry.rs",
            "LLM provider registry — reads user provider config file",
        ),
        (
            "brassclaw_llm/src/openai_codex_session.rs",
            "LLM provider session — loads Codex session token from disk",
        ),
        (
            "brassclaw_llm/src/codex_auth.rs",
            "LLM provider OAuth — loads Codex OAuth credentials from disk",
        ),
        (
            "brassclaw_llm/src/anthropic_oauth.rs",
            "LLM provider OAuth — loads Anthropic OAuth credentials from disk",
        ),
        (
            "brassclaw_reborn_config/src/config_file.rs",
            "config crate — loads config TOML files from disk (that is its purpose)",
        ),
        (
            "brassclaw_tui/src/layout.rs",
            "TUI layout — loads user layout preferences from a JSON file with default fallback",
        ),
        (
            "brassclaw_resources/src/lib.rs",
            "resources crate — reads resource governor snapshot files from disk",
        ),
        (
            "brassclaw_reborn_composition/src/skill_import.rs",
            "CLI skill-import command — reads skill markdown files from an operator-supplied directory path",
        ),
        (
            "brassclaw_engine/src/memory/retrieval_source.rs",
            "memory retrieval fallback — reads a JSONL fallback-content file for the DB-less local dev path",
        ),
    ];

    let mut violations = Vec::new();
    scan_for_direct_fs_reads(
        &crates_dir,
        &root,
        &this_test_file,
        &composition_test_file,
        forbidden_patterns,
        allowlist,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "Direct filesystem reads are forbidden in non-migration production code (Phase 9 \
         invariant: all state lives in Postgres). The migrate-from-libsql migration module \
         and the allowlisted operator-facing modules (secrets ceremony, embedded Postgres \
         management, host runtime, CLI import/export, trace contribution, build scripts) \
         are the only permitted exceptions. Violations found:\n{}",
        violations.join("\n")
    );
}

/// Walk `dir` recursively looking for Rust source files that contain any of
/// the given `patterns`. Skips:
/// - `tests/` subdirectories (test code may read fixture files)
/// - any file named `tests.rs` (inline test module included via `#[path]`)
/// - `build.rs` files (build scripts legitimately read source files)
/// - files whose path component includes `migration` (the migrate-from-libsql
///   module is the permitted exception)
/// - files whose path matches an entry in `allowlist`
/// - lines that are inside a `#[cfg(test)]` block (inline test modules)
/// - comment-only lines (lines where the first non-whitespace char is `//`)
/// - the two architecture test files passed as `skip_a` / `skip_b`
fn scan_for_direct_fs_reads(
    dir: &std::path::Path,
    root: &std::path::Path,
    skip_a: &std::path::Path,
    skip_b: &std::path::Path,
    patterns: &[(&str, &str)],
    allowlist: &[(&str, &str)],
    violations: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            // Skip `target/` and `tests/` subtrees — test code is exempt.
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name == "target" || dir_name == "tests" {
                continue;
            }
            scan_for_direct_fs_reads(&path, root, skip_a, skip_b, patterns, allowlist, violations);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        // Skip this test file and the companion composition test file.
        if path == skip_a || path == skip_b {
            continue;
        }
        // Skip `build.rs` files — build scripts legitimately read source files.
        if path.file_name().and_then(|n| n.to_str()) == Some("build.rs") {
            continue;
        }
        // Skip any file named `tests.rs` — these are inline test modules
        // included via `#[path = "tests.rs"]` and contain only test code.
        if path.file_name().and_then(|n| n.to_str()) == Some("tests.rs") {
            continue;
        }
        // Skip migration modules — they are the permitted exception.
        let path_str = path.to_string_lossy();
        if path_str.contains("migration") || path_str.contains("migrate") {
            continue;
        }
        // Skip allowlisted modules — they have a documented justification.
        let relative_str = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        if allowlist
            .iter()
            .any(|(suffix, _)| relative_str.ends_with(suffix))
        {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Strip inline `#[cfg(test)]` blocks from consideration. Handle both
        // `#[cfg(test)]\nmod ...` and `#[cfg(test)]\n#[path = "..."]\nmod ...`
        // patterns by scanning for `#[cfg(test)]` followed by `mod ` within
        // the next few lines (allowing intermediate attributes like `#[path]`).
        let production = strip_cfg_test_blocks(&contents);

        let relative = path.strip_prefix(root).unwrap_or(&path);
        for (line_number, line) in production.lines().enumerate() {
            // Skip comment-only lines.
            if line.trim_start().starts_with("//") {
                continue;
            }
            for (pattern, reason) in patterns {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{} contains `{}` ({})",
                        relative.display(),
                        line_number + 1,
                        pattern,
                        reason,
                    ));
                }
            }
        }
    }
}

/// Return the production-only slice of `contents`, with any `#[cfg(test)]`
/// inline test module stripped from the end. Handles:
/// - `#[cfg(test)]\nmod tests { ... }`
/// - `#[cfg(test)]\n#[path = "tests.rs"]\nmod tests;`
/// - any `#[cfg(test)]` followed by up to 3 intermediate attributes before `mod`
fn strip_cfg_test_blocks(contents: &str) -> &str {
    let lines: Vec<&str> = contents.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "#[cfg(test)]" {
            // Look ahead up to 4 lines for a `mod ` declaration (allowing
            // intermediate attributes like `#[path = "..."]`).
            for ahead in 1..=4 {
                let Some(peek) = lines.get(i + ahead) else {
                    break;
                };
                let trimmed = peek.trim();
                if trimmed.starts_with("mod ") {
                    // Found a cfg(test) module — strip from the `#[cfg(test)]` line.
                    let byte_idx = line.as_ptr() as usize - contents.as_ptr() as usize;
                    return &contents[..byte_idx];
                }
                if trimmed.starts_with("#[") {
                    // Intermediate attribute — keep scanning.
                    continue;
                }
                // Not an attribute and not a `mod` — this `#[cfg(test)]` is
                // on something else (e.g. a `fn`). Don't strip here.
                break;
            }
        }
    }
    contents
}
