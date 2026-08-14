use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

const INVALID_PROFILE_MESSAGE: &str = "is not a recognised";

fn reborn_bin() -> &'static str {
    env!("CARGO_BIN_EXE_brassclaw-reborn")
}

fn isolated_no_llm_command(workspace: &Path, reborn_home: &Path) -> Command {
    let mut command = Command::new(reborn_bin());
    command
        .current_dir(workspace)
        .env_clear()
        .env("HOME", workspace.join("isolated-home"))
        .env("LLM_USE_CODEX_AUTH", "false")
        .env("LLM_BACKEND", "")
        .env("LLM_MODEL", "")
        .env("OPENAI_MODEL", "")
        .env("OPENAI_CODEX_MODEL", "")
        .env("OPENAI_API_KEY", "")
        .env("ANTHROPIC_API_KEY", "")
        .env("OLLAMA_BASE_URL", "")
        .env("BRASSCLAW_REBORN_HOME", reborn_home);
    command
}

#[test]
fn help_mentions_reborn_commands() {
    let output = Command::new(reborn_bin())
        .arg("--help")
        .output()
        .expect("brassclaw-reborn --help should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Standalone BrassClaw Reborn runtime"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("channels"), "stdout: {stdout}");
    assert!(stdout.contains("completion"), "stdout: {stdout}");
    assert!(stdout.contains("config"), "stdout: {stdout}");
    assert!(stdout.contains("doctor"), "stdout: {stdout}");
    assert!(stdout.contains("extension"), "stdout: {stdout}");
    assert!(stdout.contains("hooks"), "stdout: {stdout}");
    assert!(stdout.contains("logs"), "stdout: {stdout}");
    assert!(stdout.contains("models"), "stdout: {stdout}");
    assert!(stdout.contains("profile"), "stdout: {stdout}");
    assert!(stdout.contains("repl"), "stdout: {stdout}");
    assert!(stdout.contains("run"), "stdout: {stdout}");
    // `serve` is gated behind the `webui-v2-beta` Cargo feature so a
    // default binary build does not link the beta HTTP/auth gateway.
    // The dedicated `serve_*` tests below also `#[cfg]` themselves.

    assert!(stdout.contains("serve"), "stdout: {stdout}");
    assert!(stdout.contains("skills"), "stdout: {stdout}");
}

#[test]
fn profile_list_shows_supported_profiles_without_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("runtime-profile")
        .arg("list")
        .env_clear()
        .output()
        .expect("brassclaw runtime-profile list should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw runtime profiles"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("local_dev (default)"), "stdout: {stdout}");
    assert!(stdout.contains("local_safe"), "stdout: {stdout}");
    assert!(stdout.contains("local_yolo"), "stdout: {stdout}");
    assert!(stdout.contains("hosted_safe"), "stdout: {stdout}");
    assert!(
        stdout.contains("BRASSCLAW_RUNTIME_PROFILE"),
        "stdout: {stdout}"
    );
}

#[test]
fn profile_list_json_is_stable_and_does_not_resolve_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("runtime-profile")
        .arg("list")
        .arg("--json")
        .env_clear()
        .output()
        .expect("brassclaw runtime-profile list --json should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(json["selector"], "BRASSCLAW_RUNTIME_PROFILE");
    let profiles = json["profiles"].as_array().expect("profiles array");
    assert_eq!(profiles.len(), 12);
    assert!(
        profiles
            .iter()
            .any(|profile| profile["name"] == "local_dev" && profile["default"] == true)
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile["name"] == "local_safe" && profile["default"] == false)
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile["name"] == "local_yolo" && profile["default"] == false)
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile["name"] == "hosted_safe" && profile["default"] == false)
    );
}

#[test]
fn channels_list_reports_unwired_empty_surface_without_reborn_home() {
    assert_empty_not_wired_surface(
        &["channels", "list"],
        "BrassClaw Reborn channels",
        "channels",
        "configured",
    );
}

#[test]
fn channels_list_verbose_explains_missing_reborn_registry() {
    assert_verbose_detail(
        &["channels", "list", "--verbose"],
        "Reborn channel registry is not wired yet",
    );
}

#[test]
fn channels_list_json_verbose_includes_status_details() {
    assert_json_verbose_detail(
        &["channels", "list", "--json", "--verbose"],
        "channels",
        "configured",
        "Reborn channel registry is not wired yet",
    );
}

#[test]
fn hooks_list_reports_unwired_empty_surface_without_reborn_home() {
    assert_empty_not_wired_surface(
        &["hooks", "list"],
        "BrassClaw Reborn hooks",
        "hooks",
        "configured",
    );
}

#[test]
fn hooks_list_verbose_explains_missing_reborn_registry() {
    assert_verbose_detail(
        &["hooks", "list", "--verbose"],
        "Reborn hook registry is not wired yet",
    );
}

#[test]
fn hooks_list_json_verbose_includes_status_details() {
    assert_json_verbose_detail(
        &["hooks", "list", "--json", "--verbose"],
        "hooks",
        "configured",
        "Reborn hook registry is not wired yet",
    );
}

#[test]
fn skills_list_reports_reborn_skill_data() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let v1_home = temp.path().join("v1-home");
    write_reborn_skill(&reborn_home, "catalog-helper", "catalog helper");

    let output = Command::new(reborn_bin())
        .arg("skills")
        .arg("list")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("BRASSCLAW_BASE_DIR", &v1_home)
        .output()
        .expect("brassclaw-reborn skills list should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn skills"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("configured:"), "stdout: {stdout}");
    assert!(
        stdout.contains("source: reborn-local-dev"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("- code-review (system)"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("- catalog-helper (user)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("not-wired"), "stdout: {stdout}");
    assert!(!stdout.contains("v1_state"), "stdout: {stdout}");
    assert!(
        !reborn_home
            .join("local-dev/system/skills/code-review/SKILL.md")
            .exists(),
        "skills list should report bundled skills without installing them"
    );
    assert!(
        !v1_home.exists(),
        "skills list must not create or read v1 state"
    );
}

#[test]
fn skills_list_verbose_reports_reborn_skill_details() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    write_verbose_reborn_skill(&reborn_home, "verbose-helper", "verbose helper");

    let output = Command::new(reborn_bin())
        .arg("skills")
        .arg("list")
        .arg("--verbose")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn skills list --verbose should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `profile:` was removed from skills --verbose output.
    assert!(stdout.contains("reborn_home:"), "stdout: {stdout}");
    assert!(stdout.contains("local_dev_root:"), "stdout: {stdout}");
    assert!(stdout.contains("owner_id: reborn-cli"), "stdout: {stdout}");
    assert!(stdout.contains("version: 1.2.3"), "stdout: {stdout}");
    assert!(
        stdout.contains("keywords: catalog, helper"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("tags: local-dev"), "stdout: {stdout}");
    assert!(
        stdout.contains("requires_skills: companion-helper"),
        "stdout: {stdout}"
    );
}

#[test]
fn skills_list_json_reports_reborn_skill_data() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    write_reborn_skill(&reborn_home, "json-helper", "json helper");

    let output = Command::new(reborn_bin())
        .arg("skills")
        .arg("list")
        .arg("--json")
        .arg("--verbose")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn skills list --json should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert!(
        json["configured"].as_u64().expect("configured count") > 1,
        "json: {json}"
    );
    assert_eq!(json["source"], "reborn-local-dev");
    assert_skill_source(&json, "code-review", "system");
    assert_skill_source(&json, "json-helper", "user");
    // `details.profile` was removed from skills --json output.
    assert_eq!(json["details"]["owner_id"], "reborn-cli");
    assert!(json.get("limit").is_none(), "json: {json}");
    assert!(json.get("truncated").is_none(), "json: {json}");
    assert!(json.get("status").is_none(), "json: {json}");
    assert!(json.get("v1_state").is_none(), "json: {json}");
}

fn assert_skill_source(json: &serde_json::Value, name: &str, source: &str) {
    let skills = json["skills"].as_array().expect("skills array");
    let skill = skills
        .iter()
        .find(|skill| skill["name"] == name)
        .unwrap_or_else(|| panic!("missing skill {name}: {json}"));
    assert_eq!(skill["source"], source);
}

#[test]
fn skills_list_fails_closed_for_non_local_runtime_profile() {
    // A non-local BRASSCLAW_RUNTIME_PROFILE without BRASSCLAW_PG_URL must
    // error before reaching skill listing.
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(reborn_bin())
        .arg("skills")
        .arg("list")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("BRASSCLAW_RUNTIME_PROFILE", "hosted_safe")
        .output()
        .expect("brassclaw-reborn skills list should run");

    assert!(
        !output.status.success(),
        "skills list should fail for non-local profile without BRASSCLAW_PG_URL"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires BRASSCLAW_PG_URL"),
        "stderr must mention BRASSCLAW_PG_URL, got: {stderr}"
    );
}

#[test]
fn logs_reports_unwired_surface_without_reborn_home() {
    assert_empty_not_wired_surface(&["logs"], "BrassClaw Reborn logs", "logs", "entries");
}

#[test]
fn logs_verbose_explains_missing_reborn_log_source() {
    assert_verbose_detail(&["logs", "--verbose"], "Reborn log source is not wired yet");
}

#[test]
fn logs_json_verbose_includes_status_details() {
    assert_json_verbose_detail(
        &["logs", "--json", "--verbose"],
        "logs",
        "entries",
        "Reborn log source is not wired yet",
    );
}

#[cfg(feature = "root-llm-provider")]
#[test]
fn models_list_reports_reborn_provider_catalog_without_v1_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("list")
        .env_clear()
        .env("HOME", temp.path())
        .output()
        .expect("brassclaw-reborn models list should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn LLM providers"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("providers: DB-backed"), "stdout: {stdout}");
    assert!(
        stdout.contains("active: not-configured"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("openai"), "stdout: {stdout}");
}

#[cfg(feature = "root-llm-provider")]
#[test]
fn models_status_json_reports_routes_not_configured_without_v1_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("status")
        .arg("--json")
        .env_clear()
        .env("HOME", temp.path())
        .output()
        .expect("brassclaw-reborn models status --json should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(json["routes"], "not-configured");
    assert_eq!(json["default"], serde_json::Value::Null);
    assert_eq!(json["v1_state"], "not-used");
}

#[cfg(feature = "root-llm-provider")]
#[test]
fn models_status_reads_reborn_default_llm_slot() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[llm.default]
provider_id = "openai"
model = "gpt-5-mini"
api_key_env = "OPENAI_API_KEY"
"#,
    )
    .expect("write config");

    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("status")
        .arg("--json")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn models status --json should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(json["routes"], "configured");
    assert_eq!(json["default"]["provider_id"], "openai");
    assert_eq!(json["default"]["provider_known"], true);
    assert_eq!(json["default"]["model"], "gpt-5-mini");
    assert_eq!(json["default"]["api_key_env"], "OPENAI_API_KEY");
    assert_eq!(json["v1_state"], "not-used");
}

#[cfg(feature = "root-llm-provider")]
#[test]
fn models_set_provider_writes_reborn_config_without_v1_state() {
    // `models set-provider` was deprecated and removed in favour of
    // `config set llm.default.provider_id`.  It now exits non-zero with a
    // migration hint.
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("set-provider")
        .arg("openai")
        .arg("--model")
        .arg("gpt-5-mini")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn models set-provider should run");

    assert!(
        !output.status.success(),
        "models set-provider should fail with a migration hint"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no longer supported"),
        "stderr should have migration hint: {stderr}"
    );
    assert!(
        stderr.contains("config set llm.default.provider_id"),
        "stderr should name the replacement command: {stderr}"
    );
}

#[cfg(feature = "root-llm-provider")]
#[test]
fn models_set_updates_reborn_default_model() {
    // `models set` was deprecated and removed in favour of
    // `config set llm.default.model`.  It now exits non-zero with a
    // migration hint.
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("set")
        .arg("gpt-5.3-codex")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn models set should run");

    assert!(
        !output.status.success(),
        "models set should fail with a migration hint"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no longer supported"),
        "stderr should have migration hint: {stderr}"
    );
    assert!(
        stderr.contains("config set llm.default.model"),
        "stderr should name the replacement command: {stderr}"
    );
}

#[cfg(feature = "root-llm-provider")]
#[test]
fn models_set_without_provider_fails_without_panicking() {
    // `models set` is removed; the error path for "no provider configured"
    // is now unreachable.  Confirm the command exits non-zero without panicking.
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("set")
        .arg("gpt-5.3-codex")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn models set should run");

    assert!(!output.status.success(), "models set should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no longer supported"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}

#[cfg(not(feature = "root-llm-provider"))]
#[test]
fn models_list_no_default_features_does_not_resolve_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("list")
        .env_clear()
        .output()
        .expect("brassclaw-reborn models list should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn model slots"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("v1_state: not-used"), "stdout: {stdout}");
}

#[cfg(not(feature = "root-llm-provider"))]
#[test]
fn models_status_no_default_features_does_not_resolve_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("models")
        .arg("status")
        .arg("--json")
        .env_clear()
        .output()
        .expect("brassclaw-reborn models status should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(json["routes"], "not-configured");
    assert_eq!(json["v1_state"], "not-used");
}

#[cfg(not(feature = "root-llm-provider"))]
#[test]
fn models_write_commands_report_root_llm_provider_required_without_default_features() {
    for args in [
        &["models", "set", "gpt-5.3-codex"][..],
        &["models", "set-provider", "openai"][..],
    ] {
        let output = Command::new(reborn_bin())
            .args(args)
            .env_clear()
            .output()
            .expect("brassclaw-reborn models write command should run");

        assert!(!output.status.success(), "command should fail: {args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("requires the root-llm-provider feature"),
            "stderr: {stderr}"
        );
        assert!(stderr.contains("v1_state: not-used"), "stderr: {stderr}");
        assert!(
            !stderr.contains("HOME or USERPROFILE"),
            "must not resolve Reborn home before feature error: {stderr}"
        );
    }
}

fn assert_empty_not_wired_surface(
    args: &[&str],
    title: &str,
    collection_key: &str,
    count_key: &str,
) {
    let output = Command::new(reborn_bin())
        .args(args)
        .env_clear()
        .output()
        .expect("brassclaw-reborn command should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(title), "stdout: {stdout}");
    assert!(
        stdout.contains(&format!("{count_key}: 0")),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("status: not-wired"), "stdout: {stdout}");
    assert!(stdout.contains("v1_state: not-used"), "stdout: {stdout}");

    let mut json_args = args.to_vec();
    json_args.push("--json");
    let output = Command::new(reborn_bin())
        .args(json_args)
        .env_clear()
        .output()
        .expect("brassclaw-reborn JSON command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(json[count_key], 0);
    assert_eq!(
        json[collection_key]
            .as_array()
            .expect("collection array")
            .len(),
        0
    );
    assert_eq!(json["status"], "not-wired");
    assert_eq!(json["v1_state"], "not-used");
}

fn write_reborn_skill(reborn_home: &std::path::Path, name: &str, description: &str) {
    let skill_dir = reborn_home.join("local-dev/skills").join(name);
    std::fs::create_dir_all(&skill_dir).expect("skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\nUse {name}.\n"),
    )
    .expect("skill file");
}

fn write_verbose_reborn_skill(reborn_home: &std::path::Path, name: &str, description: &str) {
    let skill_dir = reborn_home.join("local-dev/skills").join(name);
    std::fs::create_dir_all(&skill_dir).expect("skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: {name}
version: "1.2.3"
description: {description}
activation:
  keywords: ["catalog", "helper"]
  tags: ["local-dev"]
requires:
  skills: ["companion-helper"]
---
Use {name}.
"#
        ),
    )
    .expect("skill file");
}

fn assert_verbose_detail(args: &[&str], expected_detail: &str) {
    let output = Command::new(reborn_bin())
        .args(args)
        .env_clear()
        .output()
        .expect("brassclaw-reborn verbose command should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(expected_detail), "stdout: {stdout}");
}

fn assert_json_verbose_detail(
    args: &[&str],
    collection_key: &str,
    count_key: &str,
    expected_detail: &str,
) {
    let output = Command::new(reborn_bin())
        .args(args)
        .env_clear()
        .output()
        .expect("brassclaw-reborn JSON verbose command should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(json[count_key], 0);
    assert_eq!(
        json[collection_key]
            .as_array()
            .expect("collection array")
            .len(),
        0
    );
    let details = json["details"].as_array().expect("details array");
    assert!(
        details.iter().any(|detail| detail == expected_detail),
        "json: {json}"
    );
}

#[test]
fn config_path_reports_reborn_home_without_touching_v1_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let v1_base_dir = temp.path().join("v1-state");

    let output = Command::new(reborn_bin())
        .arg("config")
        .arg("path")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("BRASSCLAW_BASE_DIR", &v1_base_dir)
        .output()
        .expect("brassclaw-reborn config path should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn config path"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!("reborn_home: {}", reborn_home.display())),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("home_source: BRASSCLAW_REBORN_HOME"),
        "stdout: {stdout}"
    );
    // Profile is no longer reported by `config path` (it is set via
    // BRASSCLAW_RUNTIME_PROFILE, not stored in the config path output).
    assert!(stdout.contains("v1_state: not-used"), "stdout: {stdout}");
    assert!(
        !reborn_home.exists(),
        "config path should not create Reborn state directories"
    );
    assert!(
        !v1_base_dir.exists(),
        "config path should not create explicit v1 base directories"
    );
}

#[test]
fn config_path_reports_default_reborn_home_without_creating_directories() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join(".brassclaw").join("reborn");

    let output = Command::new(reborn_bin())
        .arg("config")
        .arg("path")
        .env_remove("BRASSCLAW_REBORN_HOME")
        .env("HOME", temp.path())
        .env_remove("USERPROFILE")
        .env_remove("BRASSCLAW_RUNTIME_PROFILE")
        .output()
        .expect("brassclaw-reborn config path should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("reborn_home: {}", reborn_home.display())),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("home_source: default"), "stdout: {stdout}");
    // Profile is no longer reported in config path output.
    assert!(
        !temp.path().join(".brassclaw").exists(),
        "config path should not create default Reborn or v1 state directories"
    );
}

#[test]
fn completion_generates_zsh_script_without_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("completion")
        .arg("--shell")
        .arg("zsh")
        .env_clear()
        .output()
        .expect("brassclaw-reborn completion should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("#compdef brassclaw-reborn"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("_brassclaw-reborn"), "stdout: {stdout}");
    assert!(
        stdout.contains("$+functions[compdef]"),
        "zsh completion should guard compdef: {stdout}"
    );
}

#[test]
fn completion_generates_bash_script_without_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("completion")
        .arg("--shell")
        .arg("bash")
        .env_clear()
        .output()
        .expect("brassclaw-reborn completion should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("_brassclaw-reborn()"), "stdout: {stdout}");
    assert!(stdout.contains("COMPREPLY"), "stdout: {stdout}");
}

#[test]
fn serve_help_mentions_host_and_port() {
    let output = Command::new(reborn_bin())
        .arg("serve")
        .arg("--help")
        .env_clear()
        .output()
        .expect("brassclaw-reborn serve --help should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"), "stdout: {stdout}");
    assert!(stdout.contains("--port"), "stdout: {stdout}");
}

#[test]
fn serve_fails_closed_when_env_bearer_token_var_is_unset() {
    // The standalone CLI's env-bearer authenticator reads the token
    // value out of the env var named by `[webui].env_token_var`
    // (defaulting to BRASSCLAW_REBORN_WEBUI_TOKEN). When that var is
    // absent the CLI must exit non-zero before binding any listener —
    // we never want a half-configured serve loop running with auth
    // disabled.
    let temp = tempfile::tempdir().expect("tempdir");

    let output = Command::new(reborn_bin())
        .arg("serve")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env_remove("BRASSCLAW_REBORN_PROFILE")
        .env_remove("BRASSCLAW_REBORN_WEBUI_TOKEN")
        .env_remove("BRASSCLAW_REBORN_WEBUI_USER_ID")
        .output()
        .expect("brassclaw-reborn serve should run");

    assert!(
        !output.status.success(),
        "serve must fail closed when the bearer token env var is unset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_WEBUI_TOKEN must be set"),
        "stderr should explain which env var is missing: {stderr}"
    );
}

#[test]
fn serve_fails_closed_when_env_user_id_var_is_unset() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(reborn_bin())
        .arg("serve")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env_remove("BRASSCLAW_REBORN_PROFILE")
        .env("BRASSCLAW_REBORN_WEBUI_TOKEN", "any-non-empty-token")
        .env_remove("BRASSCLAW_REBORN_WEBUI_USER_ID")
        .output()
        .expect("brassclaw-reborn serve should run");

    assert!(
        !output.status.success(),
        "serve must fail closed when the user-id env var is unset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_WEBUI_USER_ID must be set"),
        "stderr should name the missing user-id env var: {stderr}"
    );
}

#[test]
fn serve_rejects_malformed_host_before_webui_handoff() {
    let temp = tempfile::tempdir().expect("tempdir");

    let output = Command::new(reborn_bin())
        .arg("serve")
        .arg("--host")
        .arg("localhost:3000")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .output()
        .expect("brassclaw-reborn serve should run");

    assert!(
        !output.status.success(),
        "serve should reject malformed host"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value"), "stderr: {stderr}");
}

// Note: port `0` is intentionally accepted now — it lets the kernel
// pick a free port, which is the path the caller-level serve test
// uses to avoid hard-coding a port. The earlier zero-port rejection
// belonged to the stub serve loop that never actually bound.
//
// Banner formatting (IPv6 / IPv4 / config readout) is exercised by
// the caller-level test in
// `brassclaw_reborn_webui_ingress::tests` rather than from the binary
// smoke test, because the banner is printed AFTER env-token resolution
// + runtime build, both of which require a configured environment.

#[test]
fn run_reports_runtime_readiness_snapshot_without_touching_v1_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let home_dir = temp.path().join("home");
    let v1_base_dir = temp.path().join("v1-state");

    // `--dry-run` preserves the legacy diagnostic-only behavior: no agent
    // is started, no state directories are created. Without the flag, `run`
    // boots the live agent and would create the local-dev root.
    let output = Command::new(reborn_bin())
        .arg("run")
        .arg("--dry-run")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("HOME", &home_dir)
        .env("BRASSCLAW_BASE_DIR", &v1_base_dir)
        .env_remove("USERPROFILE")
        .env_remove("BRASSCLAW_RUNTIME_PROFILE")
        .output()
        .expect("brassclaw-reborn run should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn runtime readiness snapshot"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(reborn_home.to_str().expect("utf8 path")),
        "stdout: {stdout}"
    );
    // Profile is no longer included in run --dry-run output; readiness is
    // indicated by the planned_default_profile field instead.
    assert!(stdout.contains("v1_state: not-used"), "stdout: {stdout}");
    assert!(
        stdout.contains("runtime_driver: planned-agent-loop"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("local_runtime_shell_readiness: ready"),
        "stdout: {stdout}"
    );
    assert!(
        !reborn_home.exists(),
        "runtime readiness snapshot should not create Reborn state directories"
    );
    assert!(
        !home_dir.join(".brassclaw").exists(),
        "minimal runtime shell should not create default v1 state directories"
    );
    assert!(
        !v1_base_dir.exists(),
        "minimal runtime shell should not create explicit v1 base directories"
    );
}

#[test]
fn doctor_uses_reborn_home_override_without_touching_v1_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");

    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env_remove("BRASSCLAW_RUNTIME_PROFILE")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn doctor"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(reborn_home.to_str().expect("utf8 path")),
        "stdout: {stdout}"
    );
    // Profile is no longer reported in doctor output.
    assert!(stdout.contains("v1_state: not-used"), "stdout: {stdout}");
    assert!(
        stdout.contains("driver_registry: initialized"),
        "stdout: {stdout}"
    );
    assert!(
        !reborn_home.exists(),
        "doctor should not create state directories"
    );
}

#[test]
fn repl_help_mentions_composed_runtime() {
    let output = Command::new(reborn_bin())
        .arg("repl")
        .arg("--help")
        .env_clear()
        .output()
        .expect("brassclaw-reborn repl --help should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("composed Reborn CLI REPL"),
        "stdout: {stdout}"
    );
}

#[test]
fn repl_exit_command_exits_cleanly_without_touching_v1_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let home_dir = temp.path().join("home");
    let v1_base_dir = temp.path().join("v1-state");

    let mut child = Command::new(reborn_bin())
        .arg("repl")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("HOME", &home_dir)
        .env("BRASSCLAW_BASE_DIR", &v1_base_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn repl should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"/exit\n")
        .expect("exit command should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn repl should finish");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "stdout should stay reply-only: {stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("brassclaw-reborn: runtime started"),
        "stderr: {stderr}"
    );
    assert!(
        !home_dir.join(".brassclaw").exists(),
        "repl should not create default v1 state directories"
    );
    assert!(
        !v1_base_dir.exists(),
        "repl should not create explicit v1 base directories"
    );
}

#[test]
fn repl_resolves_codex_auth_env_without_openai_api_key() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let home_dir = temp.path().join("home");
    let codex_auth_path = temp.path().join("codex-auth.json");
    std::fs::write(
        &codex_auth_path,
        r#"{
  "auth_mode": "chatgpt",
  "tokens": {
    "access_token": "test-access-token",
    "refresh_token": "test-refresh-token"
  }
}
"#,
    )
    .expect("write codex auth fixture");

    let mut child = Command::new(reborn_bin())
        .arg("repl")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("HOME", &home_dir)
        .env("LLM_BACKEND", "openai_codex")
        .env("LLM_USE_CODEX_AUTH", "true")
        .env("CODEX_AUTH_PATH", &codex_auth_path)
        .env("OPENAI_CODEX_MODEL", "gpt-test-codex")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn repl should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"/exit\n")
        .expect("exit command should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn repl should finish");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("brassclaw-reborn: runtime started"),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains("no LLM selection configured"),
        "Codex auth should prevent stub-gateway warning: {stderr}"
    );
}

#[test]
fn repl_resolves_codex_api_key_auth_env_without_openai_api_key() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let home_dir = temp.path().join("home");
    let codex_auth_path = temp.path().join("codex-auth.json");
    std::fs::write(
        &codex_auth_path,
        r#"{
  "auth_mode": "apiKey",
  "OPENAI_API_KEY": "sk-test-codex-api-key"
}
"#,
    )
    .expect("write codex auth fixture");

    let mut child = Command::new(reborn_bin())
        .arg("repl")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("HOME", &home_dir)
        .env("LLM_BACKEND", "openai_codex")
        .env("LLM_USE_CODEX_AUTH", "true")
        .env("CODEX_AUTH_PATH", &codex_auth_path)
        .env("OPENAI_CODEX_MODEL", "gpt-test-codex")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn repl should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"/exit\n")
        .expect("exit command should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn repl should finish");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("brassclaw-reborn: runtime started"),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains("no LLM selection configured"),
        "Codex API-key auth should prevent stub-gateway warning: {stderr}"
    );
}

#[test]
fn run_rejects_codex_backend_when_auth_file_is_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let missing_codex_auth_path = temp.path().join("missing-codex-auth.json");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("LLM_BACKEND", "openai_codex")
        .env("CODEX_AUTH_PATH", &missing_codex_auth_path)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "missing Codex auth should fail; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Authentication failed for provider 'openai_codex'"),
        "stderr should report Codex auth failure; got: {stderr}"
    );
    assert!(
        !stderr.contains(&missing_codex_auth_path.display().to_string()),
        "stderr should not leak the Codex auth path: {stderr}"
    );
}

#[test]
fn repl_help_command_prints_repl_commands_and_exits_on_exit() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut child = Command::new(reborn_bin())
        .arg("repl")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("HOME", temp.path().join("home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn repl should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"/help\n/quit\n")
        .expect("repl commands should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn repl should finish");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Reborn REPL commands:"), "stderr: {stderr}");
    assert!(stderr.contains("/exit"), "stderr: {stderr}");
    assert!(stderr.contains("/quit"), "stderr: {stderr}");
}

#[test]
fn run_help_command_prints_repl_commands_and_exits_on_quit() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut child = Command::new(reborn_bin())
        .arg("run")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("HOME", temp.path().join("home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn run should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"/help\n/quit\n")
        .expect("run repl commands should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn run should finish");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "stdout should stay reply-only: {stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Reborn REPL commands:"), "stderr: {stderr}");
    assert!(stderr.contains("/exit"), "stderr: {stderr}");
    assert!(stderr.contains("/quit"), "stderr: {stderr}");
}

#[test]
fn repl_piped_message_exits_nonzero_when_runtime_does_not_produce_reply() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut child = Command::new(reborn_bin())
        .arg("repl")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("HOME", temp.path().join("home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn repl should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\n")
        .expect("prompt should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn repl should finish");

    assert!(
        !output.status.success(),
        "repl should fail when the runtime cannot produce assistant text"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "stdout should stay reply-only: {stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reborn run did not produce an assistant reply"),
        "stderr: {stderr}"
    );
}

#[test]
fn run_message_exits_nonzero_when_runtime_does_not_produce_reply() {
    let temp = tempfile::tempdir().expect("tempdir");

    let output = Command::new(reborn_bin())
        .arg("run")
        .arg("--message")
        .arg("hello")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("HOME", temp.path().join("home"))
        .output()
        .expect("brassclaw-reborn run --message should run");

    assert!(
        !output.status.success(),
        "run --message should fail when the runtime cannot produce assistant text"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "stdout should stay reply-only: {stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reborn run did not produce an assistant reply"),
        "stderr: {stderr}"
    );
}

#[test]
fn run_piped_stdin_exits_nonzero_when_runtime_does_not_produce_reply() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut child = Command::new(reborn_bin())
        .arg("run")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("HOME", temp.path().join("home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brassclaw-reborn run should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"  hello  \n")
        .expect("prompt should be written");
    let output = child
        .wait_with_output()
        .expect("brassclaw-reborn run should finish");

    assert!(
        !output.status.success(),
        "piped run should fail when the runtime cannot produce assistant text"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "stdout should stay reply-only: {stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reborn run did not produce an assistant reply"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_default_home_is_reborn_scoped_and_dry_run() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join(".brassclaw").join("reborn");

    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env_remove("BRASSCLAW_REBORN_HOME")
        .env("HOME", temp.path())
        .env_remove("USERPROFILE")
        .env_remove("BRASSCLAW_RUNTIME_PROFILE")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(reborn_home.to_str().expect("utf8 path")),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("home_source: default"), "stdout: {stdout}");
    // Profile is no longer reported in doctor output.
    assert!(
        !temp.path().join(".brassclaw").exists(),
        "doctor should not create default Reborn or v1 state directories"
    );
}

#[test]
fn doctor_reports_explicit_profile() {
    let temp = tempfile::tempdir().expect("tempdir");

    // Profile is selected via BRASSCLAW_RUNTIME_PROFILE (not the deprecated
    // BRASSCLAW_REBORN_PROFILE). Doctor no longer prints the profile in its
    // output — selecting a valid profile just succeeds silently.
    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("BRASSCLAW_RUNTIME_PROFILE", "local_safe")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn run_reports_explicit_profile() {
    let temp = tempfile::tempdir().expect("tempdir");

    // Profile is now selected via BRASSCLAW_RUNTIME_PROFILE; --dry-run
    // exercises the boot path without booting the agent.
    let output = Command::new(reborn_bin())
        .arg("run")
        .arg("--dry-run")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("BRASSCLAW_RUNTIME_PROFILE", "local_safe")
        .output()
        .expect("brassclaw-reborn run should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The runtime readiness snapshot is emitted; profile is not printed.
    assert!(
        stdout.contains("BrassClaw Reborn runtime readiness snapshot"),
        "stdout: {stdout}"
    );
}

#[test]
fn doctor_rejects_invalid_profile() {
    let temp = tempfile::tempdir().expect("tempdir");

    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("BRASSCLAW_RUNTIME_PROFILE", "prod")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        !output.status.success(),
        "doctor should reject invalid profile"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(INVALID_PROFILE_MESSAGE), "stderr: {stderr}");
}

#[test]
fn doctor_rejects_empty_profile_override() {
    let temp = tempfile::tempdir().expect("tempdir");

    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("BRASSCLAW_RUNTIME_PROFILE", "")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        !output.status.success(),
        "doctor should reject empty profile"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(INVALID_PROFILE_MESSAGE), "stderr: {stderr}");
}

#[test]
fn run_rejects_invalid_profile() {
    let temp = tempfile::tempdir().expect("tempdir");

    let output = Command::new(reborn_bin())
        .arg("run")
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env("BRASSCLAW_RUNTIME_PROFILE", "prod")
        .output()
        .expect("brassclaw-reborn run should run");

    assert!(
        !output.status.success(),
        "run should reject invalid profile"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(INVALID_PROFILE_MESSAGE), "stderr: {stderr}");
}

#[test]
fn run_rejects_reborn_home_equal_to_explicit_v1_base_dir() {
    let temp = tempfile::tempdir().expect("tempdir");
    let v1_root = temp.path().join("v1-state");

    let output = Command::new(reborn_bin())
        .arg("run")
        .env("BRASSCLAW_REBORN_HOME", &v1_root)
        .env("BRASSCLAW_BASE_DIR", &v1_root)
        .output()
        .expect("brassclaw-reborn run should run");

    assert!(!output.status.success(), "run should reject v1 root");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_HOME must not point at the v1 BrassClaw state root"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_rejects_reborn_home_equal_to_explicit_v1_base_dir() {
    let temp = tempfile::tempdir().expect("tempdir");
    let v1_root = temp.path().join("v1-state");

    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env("BRASSCLAW_REBORN_HOME", &v1_root)
        .env("BRASSCLAW_BASE_DIR", &v1_root)
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(!output.status.success(), "doctor should reject v1 root");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_HOME must not point at the v1 BrassClaw state root"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_rejects_reborn_home_equal_to_relative_explicit_v1_base_dir() {
    let temp = tempfile::tempdir().expect("tempdir");
    let v1_root = temp.path().join("v1-state");

    let output = Command::new(reborn_bin())
        .arg("doctor")
        .current_dir(temp.path())
        .env("BRASSCLAW_REBORN_HOME", &v1_root)
        .env("BRASSCLAW_BASE_DIR", "v1-state")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(!output.status.success(), "doctor should reject v1 root");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_HOME must not point at the v1 BrassClaw state root"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_rejects_empty_reborn_home_override() {
    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", "")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(!output.status.success(), "doctor should reject empty home");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_HOME must not be empty"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_rejects_relative_reborn_home_override() {
    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", "relative/reborn")
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        !output.status.success(),
        "doctor should reject relative home"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_HOME must be an absolute path"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_rejects_missing_home_for_default_reborn_home() {
    let output = Command::new(reborn_bin())
        .arg("doctor")
        .env_clear()
        .output()
        .expect("brassclaw-reborn doctor should run");

    assert!(
        !output.status.success(),
        "doctor should reject missing home"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("HOME or USERPROFILE must be set"),
        "stderr: {stderr}"
    );
}

// ─── Boot-config TOML + provider catalog ─────────────────────────────────────
//
// `config init` now writes to PostgreSQL (requires embedded or external
// Postgres). These tests are skipped in environments without `initdb`.

#[test]
#[ignore = "requires embedded Postgres (initdb); run manually with BRASSCLAW_PG_URL set"]
fn config_init_writes_both_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let output = Command::new(reborn_bin())
        .args(["config", "init", "--yes", "--no-llm"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn config init should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires embedded Postgres (initdb); run manually with BRASSCLAW_PG_URL set"]
fn config_init_refuses_to_clobber_without_force() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");

    let first = Command::new(reborn_bin())
        .args(["config", "init", "--yes", "--no-llm"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("first init should run");
    assert!(first.status.success());

    // Second run without --yes should detect boot.initialized and refuse.
    let second = Command::new(reborn_bin())
        .args(["config", "init"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("second init should run");
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("already initialized"),
        "second init without --yes should report already initialized; got: {stdout}"
    );
}

#[test]
#[ignore = "requires embedded Postgres (initdb); run manually with BRASSCLAW_PG_URL set"]
fn config_init_preflights_both_targets_before_writing() {
    // Placeholder: preflight logic is now inside the Postgres-backed init.
    // Integration test coverage lives in brassclaw_reborn_composition.
}

#[test]
#[ignore = "requires embedded Postgres (initdb); run manually with BRASSCLAW_PG_URL set"]
fn config_init_with_force_overwrites() {
    // Placeholder: --force / overwrite is now --yes on the DB-backed wizard.
    // Integration test coverage lives in brassclaw_reborn_composition.
}

#[test]
fn config_path_reports_file_presence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");

    // `config path` now reports the config.toml path as always present
    // (DB-backed) since the DB is authoritative — the line reads
    // "config.toml (read-only at boot; settings are DB-backed)".
    let output = Command::new(reborn_bin())
        .args(["config", "path"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("config path runs");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("config_file"),
        "stdout should mention config_file: {stdout}"
    );
    assert!(
        stdout.contains("providers: DB-backed"),
        "stdout should report DB-backed providers: {stdout}"
    );
}

#[test]
fn run_with_inline_secret_in_config_fails_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    let bad_config = r#"
[llm.default]
provider_id = "openai"
api_key_env = "sk-proj-1234567890abcdef12345678"
"#;
    std::fs::write(reborn_home.join("config.toml"), bad_config).expect("write bad config");

    let output = isolated_no_llm_command(temp.path(), &reborn_home)
        .args(["run", "-m", "ping"])
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "inline secret must cause failure; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inline secret") || stderr.contains("secret"),
        "stderr should mention inline secret rejection; got: {stderr}"
    );
}

#[test]
fn run_warns_when_falling_back_to_stub_gateway() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace dir");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");

    let output = isolated_no_llm_command(&workspace, &reborn_home)
        .args(["run", "-m", "ping"])
        .output()
        .expect("brassclaw-reborn run should not crash");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no LLM selection configured") && stderr.contains("Runs will fail"),
        "stderr should warn about degraded stub-gateway boot; got: {stderr}"
    );
    assert!(
        reborn_home
            .join("local-dev/system/skills/code-review/SKILL.md")
            .is_file(),
        "runtime bootstrap should install bundled Reborn skills"
    );
}

#[test]
fn run_confirm_host_access_flag_gates_local_dev_yolo() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = local_yolo_command(&temp, &["run", "-m", "ping"])
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(!missing.status.success(), "missing confirmation must fail");
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing_stderr.contains("requires explicit disclosure acknowledgement"),
        "stderr should require disclosure acknowledgement; got: {missing_stderr}"
    );

    let confirmed = local_yolo_command(&temp, &["run", "--confirm-host-access", "-m", "ping"])
        .output()
        .expect("brassclaw-reborn run should not crash");
    let confirmed_stderr = String::from_utf8_lossy(&confirmed.stderr);
    assert!(
        !confirmed_stderr.contains("requires explicit disclosure acknowledgement")
            && !confirmed_stderr.contains("requires --confirm-host-access"),
        "confirmed run should pass the host-access gate; got: {confirmed_stderr}"
    );
}

#[test]
fn run_confirm_host_access_requires_home_or_userprofile() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("reborn home");

    let output = Command::new(reborn_bin())
        .args(["run", "--confirm-host-access", "-m", "ping"])
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("BRASSCLAW_RUNTIME_PROFILE", "local_yolo")
        .output()
        .expect("brassclaw-reborn run should not crash");

    assert!(!output.status.success(), "missing host home must fail"); // safety: test-only assertion.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        /* safety: test-only assertion. */
        stderr.contains("HOME or USERPROFILE must be set"),
        "stderr should require a host home root; got: {stderr}"
    );
}

#[test]
fn run_confirm_host_access_uses_userprofile_when_home_is_absent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let host_home = temp.path().join("host-home");
    std::fs::create_dir_all(&reborn_home).expect("reborn home");
    std::fs::create_dir_all(&host_home).expect("host home");

    let output = Command::new(reborn_bin())
        .args(["run", "--confirm-host-access", "-m", "ping"])
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("BRASSCLAW_RUNTIME_PROFILE", "local_yolo")
        .env("USERPROFILE", &host_home)
        .output()
        .expect("brassclaw-reborn run should not crash");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("HOME or USERPROFILE must be set")
            && !stderr.contains("requires explicit disclosure acknowledgement")
            && !stderr.contains("requires --confirm-host-access"),
        "confirmed run should use USERPROFILE and pass the host-access gate; got: {stderr}"
    );
}

#[test]
fn repl_confirm_host_access_flag_gates_local_dev_yolo() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = local_yolo_command(&temp, &["repl"])
        .stdin(Stdio::null())
        .output()
        .expect("brassclaw-reborn repl should not crash");
    assert!(!missing.status.success(), "missing confirmation must fail");
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing_stderr.contains("requires explicit disclosure acknowledgement"),
        "stderr should require disclosure acknowledgement; got: {missing_stderr}"
    );

    let confirmed = local_yolo_command(&temp, &["repl", "--confirm-host-access"])
        .stdin(Stdio::null())
        .output()
        .expect("brassclaw-reborn repl should not crash");
    let confirmed_stderr = String::from_utf8_lossy(&confirmed.stderr);
    assert!(
        !confirmed_stderr.contains("requires explicit disclosure acknowledgement")
            && !confirmed_stderr.contains("requires --confirm-host-access"),
        "confirmed repl should pass the host-access gate; got: {confirmed_stderr}"
    );
}

#[test]
fn serve_confirm_host_access_flag_gates_local_dev_yolo() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = local_yolo_command(&temp, &["serve"])
        .output()
        .expect("brassclaw-reborn serve should not crash");
    assert!(!missing.status.success(), "missing confirmation must fail");
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing_stderr.contains("requires explicit disclosure acknowledgement"),
        "stderr should require disclosure acknowledgement; got: {missing_stderr}"
    );

    let confirmed = local_yolo_command(&temp, &["serve", "--confirm-host-access"])
        .output()
        .expect("brassclaw-reborn serve should not crash");
    assert!(
        !confirmed.status.success(),
        "serve still needs webui token config"
    );
    let confirmed_stderr = String::from_utf8_lossy(&confirmed.stderr);
    assert!(
        !confirmed_stderr.contains("requires explicit disclosure acknowledgement")
            && !confirmed_stderr.contains("requires --confirm-host-access"),
        "confirmed serve should pass the host-access gate; got: {confirmed_stderr}"
    );
    assert!(
        confirmed_stderr.contains("BRASSCLAW_REBORN_WEBUI_TOKEN"),
        "confirmed serve should reach WebUI token resolution; got: {confirmed_stderr}"
    );
}

#[test]
fn serve_confirmed_local_dev_yolo_rejects_non_loopback_cli_host() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = local_yolo_command(
        &temp,
        &["serve", "--confirm-host-access", "--host", "0.0.0.0"],
    )
    .env("BRASSCLAW_REBORN_WEBUI_TOKEN", "test-token")
    .env("BRASSCLAW_REBORN_WEBUI_USER_ID", "test-user")
    .output()
    .expect("brassclaw-reborn serve should not crash");

    assert!(
        !output.status.success(),
        "non-loopback confirmed yolo serve must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refuses non-loopback listener 0.0.0.0")
            && stderr.contains("trusted-laptop host access"),
        "stderr should reject non-loopback trusted-laptop access; got: {stderr}"
    );
}

#[test]
fn serve_confirmed_local_dev_yolo_rejects_non_loopback_config_host() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("reborn home");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[webui]
listen_host = "0.0.0.0"
"#,
    )
    .expect("write config");

    let output = local_yolo_command(&temp, &["serve", "--confirm-host-access"])
        .env("BRASSCLAW_REBORN_WEBUI_TOKEN", "test-token")
        .env("BRASSCLAW_REBORN_WEBUI_USER_ID", "test-user")
        .output()
        .expect("brassclaw-reborn serve should not crash");

    assert!(
        !output.status.success(),
        "non-loopback confirmed yolo serve from config must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refuses non-loopback listener 0.0.0.0")
            && stderr.contains("trusted-laptop host access"),
        "stderr should reject config-driven non-loopback trusted-laptop access; got: {stderr}"
    );
}

#[test]
fn serve_local_dev_allows_non_loopback_without_trusted_laptop_access() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(reborn_bin())
        .args(["serve", "--host", "0.0.0.0", "--port", "0"])
        .env("BRASSCLAW_REBORN_HOME", temp.path().join("reborn-home"))
        .env_remove("BRASSCLAW_REBORN_PROFILE")
        .env_remove("BRASSCLAW_REBORN_WEBUI_TOKEN")
        .env_remove("BRASSCLAW_REBORN_WEBUI_USER_ID")
        .output()
        .expect("brassclaw-reborn serve should not crash");

    assert!(
        !output.status.success(),
        "serve should still fail closed on missing WebUI token"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BRASSCLAW_REBORN_WEBUI_TOKEN must be set"),
        "ordinary local-dev serve should reach WebUI token validation; got: {stderr}"
    );
    assert!(
        !stderr.contains("trusted-laptop host access"),
        "ordinary local-dev serve should not trigger the trusted-laptop listener refusal; got: {stderr}"
    );
}

#[test]
fn run_honors_boot_profile_from_config_file() {
    // Profile is no longer stored in config.toml — [boot].profile was removed
    // in Phase 11. Profile selection uses BRASSCLAW_RUNTIME_PROFILE env var.
    // A non-local profile without BRASSCLAW_PG_URL must fail-closed.
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_RUNTIME_PROFILE", "hosted_safe")
        .env_remove("BRASSCLAW_PG_URL")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "non-local profile without PG_URL should fail; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires BRASSCLAW_PG_URL"),
        "stderr should explain PG_URL requirement; got: {stderr}"
    );
}

#[test]
fn run_rejects_inline_secret_in_provider_id_without_echoing_value() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    let secret = "sk-proj-1234567890abcdef1234567890";
    std::fs::write(
        reborn_home.join("config.toml"),
        format!(
            r#"
[llm.default]
provider_id = " {secret} "
"#
        ),
    )
    .expect("write config");

    let output = isolated_no_llm_command(temp.path(), &reborn_home)
        .args(["run", "-m", "ping"])
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(!output.status.success(), "inline secret must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inline secret") || stderr.contains("secret"),
        "stderr should mention secret rejection; got: {stderr}"
    );
    assert!(
        !stderr.contains(secret),
        "stderr must not echo pasted secret; got: {stderr}"
    );
}

#[test]
fn run_accepts_configured_cli_tenant_and_agent_identity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace dir");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[identity]
tenant = "reborn-cli"
default_agent = "reborn-cli-agent"
default_owner = "operator"
"#,
    )
    .expect("write config");

    let output = isolated_no_llm_command(&workspace, &reborn_home)
        .args(["run", "-m", "ping"])
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "run should still fail without a model gateway"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reborn run did not produce an assistant reply"),
        "stderr should reach normal runtime failure; got: {stderr}"
    );
    assert!(
        !stderr.contains("tenant") && !stderr.contains("default_agent"),
        "tenant/default_agent should be accepted by CLI identity wiring; got: {stderr}"
    );
}

#[test]
fn run_rejects_unsupported_identity_project_scope_field() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[identity]
tenant = "reborn-cli"
default_agent = "reborn-cli-agent"
default_owner = "operator"
default_project = "project-alpha"
"#,
    )
    .expect("write config");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "unsupported project scope must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[identity]")
            && stderr.contains("default_project")
            && stderr.contains("not wired"),
        "stderr should explain unsupported project scope; got: {stderr}"
    );
}

#[test]
fn run_rejects_unsupported_policy_driver_and_harness_sections() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[policy]
default_approval_policy = "ask_always"
"#,
    )
    .expect("write config");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(!output.status.success(), "unsupported policy must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[policy]") && stderr.contains("not wired"),
        "stderr should explain unsupported section; got: {stderr}"
    );
}

#[test]
fn run_rejects_malformed_explicit_provider_overlay() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[llm.default]
provider_id = "openai"
"#,
    )
    .expect("write config");
    std::fs::write(reborn_home.join("providers.json"), "not json").expect("write providers");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(!output.status.success(), "malformed overlay must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("provider catalog") || stderr.contains("providers.json"),
        "stderr should explain provider catalog load failure; got: {stderr}"
    );
}

#[test]
fn run_rejects_empty_required_api_key_env() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[llm.default]
provider_id = "empty-key-provider"
"#,
    )
    .expect("write config");
    std::fs::write(
        reborn_home.join("providers.json"),
        r#"[
  {
    "id": "empty-key-provider",
    "protocol": "open_ai_completions",
    "api_key_env": "REBORN_TEST_EMPTY_KEY",
    "api_key_required": true,
    "model_env": "REBORN_TEST_MODEL",
    "default_model": "test-model",
    "description": "test provider"
  }
]
"#,
    )
    .expect("write providers");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .env("REBORN_TEST_EMPTY_KEY", "")
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(!output.status.success(), "empty API key must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("REBORN_TEST_EMPTY_KEY") && stderr.contains("requires API key env var"),
        "stderr should treat empty key as unset; got: {stderr}"
    );
}

#[test]
fn run_rejects_zero_runner_heartbeat_interval() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[runner]
heartbeat_interval_secs = 0
"#,
    )
    .expect("write config");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "zero heartbeat interval must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("heartbeat_interval_secs") && stderr.contains("greater than 0"),
        "stderr should explain heartbeat interval rejection; got: {stderr}"
    );
}

#[test]
fn run_rejects_zero_runner_poll_interval() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[runner]
poll_interval_ms = 0
"#,
    )
    .expect("write config");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(!output.status.success(), "zero poll interval must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("poll_interval_ms") && stderr.contains("greater than 0"),
        "stderr should explain poll interval rejection; got: {stderr}"
    );
}

#[test]
fn run_resolves_provider_from_config_and_demands_api_key_env() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    let cfg = r#"
[llm.default]
provider_id = "openai"
model = "gpt-4o-mini"
api_key_env = "REBORN_TEST_UNSET_BC8F4D_KEY"
"#;
    std::fs::write(reborn_home.join("config.toml"), cfg).expect("write config");

    let output = Command::new(reborn_bin())
        .args(["run", "-m", "ping"])
        .env_remove("USERPROFILE")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OLLAMA_BASE_URL")
        .env_remove("REBORN_TEST_UNSET_BC8F4D_KEY")
        .env("BRASSCLAW_REBORN_HOME", &reborn_home)
        .output()
        .expect("brassclaw-reborn run should not crash");
    assert!(
        !output.status.success(),
        "missing api key must fail; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("REBORN_TEST_UNSET_BC8F4D_KEY"),
        "stderr should name the unset env var; got: {stderr}"
    );
}

fn local_yolo_command(temp: &tempfile::TempDir, args: &[&str]) -> Command {
    let reborn_home = temp.path().join("reborn-home");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&reborn_home).expect("reborn home");
    std::fs::create_dir_all(&home).expect("home");
    let mut command = Command::new(reborn_bin());
    command
        .args(args)
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", reborn_home)
        .env("BRASSCLAW_RUNTIME_PROFILE", "local_yolo")
        .env("HOME", home);
    command
}
