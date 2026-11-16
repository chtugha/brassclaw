#![cfg(test)]

use std::collections::HashMap;

// Trigger-poller interval written in test configs to exercise config parsing.
const TEST_TRIGGER_POLL_INTERVAL_SECS: u64 = 42;

use brassclaw_reborn_config::RebornBootConfig;

use super::test_env::{EnvGuard, lock_trigger_env};

use super::{
    RUNTIME_PROFILE_ENV, RuntimeInputCaller, RuntimeInputOptions, build_runtime_input,
    build_runtime_input_with_options, resolve_google_oauth_config,
};

fn clear_trigger_poller_env() -> (EnvGuard, EnvGuard) {
    (
        EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_ENABLED"),
        EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS"),
    )
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn run_postgres_transition_rejects_missing_contract_before_server_start() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("reborn-home");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(home.clone().into_os_string()), None, None)
            .expect("boot config");
    for missing_services in [true, false] {
        let mut input = brassclaw_reborn_composition::RebornRuntimeInput::from_services(
            brassclaw_reborn_composition::RebornBuildInput::local_dev(
                "owner",
                temp.path().join("local"),
            ),
        );
        if missing_services {
            input.services = None;
        }
        let error = super::start_postgres_for_run(input, &config)
            .await
            .err()
            .expect("an existing service contract and policy are mandatory");
        assert!(
            error.to_string().contains(if missing_services {
                "existing services input"
            } else {
                "already resolved runtime policy"
            }),
            "{error:#}"
        );
        assert!(
            !home.join("postgres").exists(),
            "invalid input must not start a server"
        );
    }
}

#[cfg(feature = "postgres")]
#[test]
fn run_postgres_transition_failure_closes_owned_server() {
    let _lock = lock_trigger_env();
    let _external = EnvGuard::clear("BRASSCLAW_PG_URL");
    let _database = EnvGuard::clear("DATABASE_URL");
    let _listen = EnvGuard::set("BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES", "127.0.0.1");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("own unused port");
    let address = listener.local_addr().expect("reserved address");
    drop(listener);
    let _port = EnvGuard::set("BRASSCLAW_EMBEDDED_PG_PORT", &address.port().to_string());
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("reborn-home");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(home.clone().into_os_string()), None, None)
            .expect("boot config");
    let input = brassclaw_reborn_composition::RebornRuntimeInput::from_services(
        brassclaw_reborn_composition::RebornBuildInput::disabled("owner").with_runtime_policy(
            brassclaw_reborn_composition::local_dev_runtime_policy().expect("resolved policy"),
        ),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let error = runtime
        .block_on(super::start_postgres_for_run(input, &config))
        .err()
        .expect("disabled storage cannot be replaced after startup");
    assert!(
        format!("{error:#}").contains("requires an existing LocalDev input"),
        "{error:#}"
    );
    let data = home.join("postgres/data");
    assert!(
        data.join("PG_VERSION").exists(),
        "real owned server must have been initialized"
    );
    assert!(
        !data.join("postmaster.pid").exists(),
        "owned server must have stopped"
    );
    assert!(
        std::net::TcpStream::connect(address).is_err(),
        "owned PG port must be closed"
    );
}

#[test]
fn build_runtime_input_maps_configured_cli_identity() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[identity]
tenant = "custom-tenant"
default_agent = "custom-agent"
default_owner = "custom-owner"
"#,
    )
    .expect("write config");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let runtime_input =
        build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

    assert_eq!(runtime_input.identity.tenant_id, "custom-tenant");
    assert_eq!(runtime_input.identity.agent_id, "custom-agent");
    assert_eq!(runtime_input.identity.source_binding_id, "reborn-cli");
    assert_eq!(runtime_input.identity.reply_target_binding_id, "reborn-cli");
}

#[test]
fn build_runtime_input_maps_regex_skill_activation_config() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[skills]
regex_activation_enabled = false
"#,
    )
    .expect("write config");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let runtime_input =
        build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

    assert!(!runtime_input.regex_skill_activation_enabled);
}

#[test]
fn build_runtime_input_rejects_local_dev_yolo_without_host_access_confirmation() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();
    // Use BRASSCLAW_RUNTIME_PROFILE env var to request yolo mode.
    let _profile = EnvGuard::set(RUNTIME_PROFILE_ENV, "local_yolo");

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let error = match build_runtime_input(&config, RuntimeInputCaller::Run) {
        Ok(_) => panic!("local-dev-yolo requires confirmation"),
        Err(error) => error,
    };

    assert!(format!("{error:#}").contains("requires explicit disclosure acknowledgement"));
}

#[test]
fn build_runtime_input_accepts_confirmed_local_dev_yolo_profile() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();
    // Use BRASSCLAW_RUNTIME_PROFILE env var to request yolo mode.
    let _profile = EnvGuard::set(RUNTIME_PROFILE_ENV, "local_yolo");

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let runtime_input = build_runtime_input_with_options(
        &config,
        RuntimeInputCaller::Run,
        RuntimeInputOptions {
            confirm_host_access: true,
        },
    )
    .expect("runtime input");
    assert!(runtime_input.grants_trusted_laptop_access());
    let services = runtime_input.services.expect("services input");
    let policy = services.runtime_policy().expect("runtime policy");

    assert_eq!(
        policy.filesystem_backend.as_str(),
        "host_workspace_and_home"
    );
    assert_eq!(policy.secret_mode.as_str(), "inherited_env");
}

// Regression for the review point that `serve` rejected legitimate
// `[identity].default_project` configs at runtime-input build time
// because the unsupported-section check was shared with `run` / `repl`.
// `serve` consumes the value, `run` does not — the discriminator
// ensures both branches do the right thing.
#[test]
fn build_runtime_input_for_run_rejects_default_project() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[identity]
default_project = "project-alpha"
"#,
    )
    .expect("write config");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let err = build_runtime_input(&config, RuntimeInputCaller::Run)
        .err()
        .expect("run must reject default_project");
    assert!(
        err.to_string().contains("default_project"),
        "error must mention the rejected field, got: {err}",
    );
}

#[test]
fn build_runtime_input_for_run_rejects_default_project_when_trigger_poller_enabled() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[identity]
default_project = "project-alpha"

[trigger_poller]
enabled = true
"#,
    )
    .expect("write config");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let err = build_runtime_input(&config, RuntimeInputCaller::Run)
        .err()
        .expect("run must reject default_project even when trigger poller is enabled");
    assert!(
        err.to_string().contains("default_project"),
        "error must mention the rejected field, got: {err}",
    );
}

#[test]
fn build_runtime_input_for_serve_accepts_default_project() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[identity]
default_project = "project-alpha"
"#,
    )
    .expect("write config");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let _runtime_input = build_runtime_input(&config, RuntimeInputCaller::Serve)
        .expect("serve must accept default_project");
}

#[test]
fn build_runtime_input_maps_trigger_poller_enabled_config() {
    let _lock = lock_trigger_env();
    let (_enabled, _interval) = clear_trigger_poller_env();

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
            reborn_home.join("config.toml"),
            format!(
                "[trigger_poller]\nenabled = true\npoll_interval_secs = {TEST_TRIGGER_POLL_INTERVAL_SECS}\n"
            ),
        )
        .expect("write config");
    let config =
        RebornBootConfig::resolve_from_env_parts(Some(reborn_home.into_os_string()), None, None)
            .expect("boot config");

    let input = build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

    assert!(
        input.trigger_poller.enabled,
        "[trigger_poller] enabled=true in config must reach runtime_input.trigger_poller.enabled"
    );
    assert_eq!(
        input.trigger_poller.worker.poll_interval,
        std::time::Duration::from_secs(TEST_TRIGGER_POLL_INTERVAL_SECS),
        "config poll_interval_secs must reach worker.poll_interval"
    );
}

#[test]
fn build_runtime_input_env_enables_trigger_poller_with_no_config_section() {
    // No [trigger_poller] in config; env var enables → input.trigger_poller.enabled must be true.
    let _lock = lock_trigger_env();
    let _enabled = EnvGuard::set("BRASSCLAW_TRIGGER_POLLER_ENABLED", "true");
    let _interval = EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS");

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    // No config.toml written → no [trigger_poller] section at all.

    let config = RebornBootConfig::resolve_from_env_parts(
        Some(reborn_home.to_string_lossy().to_string().into()),
        None,
        None,
    )
    .expect("boot config");

    let input = build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

    assert!(
        input.trigger_poller.enabled,
        "BRASSCLAW_TRIGGER_POLLER_ENABLED=true must reach input.trigger_poller.enabled through build_runtime_input"
    );
}

#[test]
fn build_runtime_input_env_interval_overrides_config_interval() {
    // Config says interval=15s, env says interval=45s → env must win at the caller boundary.
    let _lock = lock_trigger_env();
    let _enabled = EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_ENABLED");
    let _interval = EnvGuard::set("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS", "45");

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");
    std::fs::write(
        reborn_home.join("config.toml"),
        r#"
[trigger_poller]
enabled = true
poll_interval_secs = 15
"#,
    )
    .expect("write config");

    let config = RebornBootConfig::resolve_from_env_parts(
        Some(reborn_home.to_string_lossy().to_string().into()),
        None,
        None,
    )
    .expect("boot config");

    let input = build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

    assert_eq!(
        input.trigger_poller.worker.poll_interval,
        std::time::Duration::from_secs(45),
        "env BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS=45 must override config poll_interval_secs=15 through build_runtime_input"
    );
}

#[test]
fn build_runtime_input_rejects_invalid_trigger_poller_enabled_env() {
    // Invalid env value (`yes`) must error out through build_runtime_input,
    // not slip through to the runtime input. Closes the caller-level gap
    // for the error path; previous tests covered only happy/override paths.
    let _lock = lock_trigger_env();
    let _enabled = EnvGuard::set("BRASSCLAW_TRIGGER_POLLER_ENABLED", "yes");
    let _interval = EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS");

    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    std::fs::create_dir_all(&reborn_home).expect("mkdir");

    let config = RebornBootConfig::resolve_from_env_parts(
        Some(reborn_home.to_string_lossy().to_string().into()),
        None,
        None,
    )
    .expect("boot config");

    let err = match build_runtime_input(&config, RuntimeInputCaller::Run) {
        Ok(_) => panic!(
            "invalid BRASSCLAW_TRIGGER_POLLER_ENABLED must propagate as Err through build_runtime_input"
        ),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("BRASSCLAW_TRIGGER_POLLER_ENABLED"),
        "caller-level error must surface the env var name, got: {err}",
    );
}

#[test]
fn resolve_google_oauth_config_returns_none_when_no_vars_set() {
    let config = resolve_google_oauth_config(|_| None).expect("empty env should not fail setup");

    assert!(config.is_none());
}

#[test]
fn resolve_google_oauth_config_errors_when_client_id_missing() {
    let vars = HashMap::from([(
        "BRASSCLAW_REBORN_GOOGLE_OAUTH_REDIRECT_URI",
        "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
    )]);

    let error = resolve_google_oauth_config(|name| vars.get(name).map(|value| value.to_string()))
        .expect_err("redirect-only Google OAuth config must fail closed");

    assert!(error.to_string().contains("GOOGLE_CLIENT_ID"));
}

#[test]
fn resolve_google_oauth_config_prefers_reborn_prefixed_vars() {
    let vars = HashMap::from([
        (
            "BRASSCLAW_REBORN_GOOGLE_CLIENT_ID",
            "reborn-client.apps.googleusercontent.com",
        ),
        (
            "BRASSCLAW_REBORN_GOOGLE_CLIENT_SECRET",
            "reborn-client-secret",
        ),
        (
            "BRASSCLAW_REBORN_GOOGLE_OAUTH_REDIRECT_URI",
            "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
        ),
        (
            "BRASSCLAW_REBORN_GOOGLE_HOSTED_DOMAIN_HINT",
            "reborn.example.com",
        ),
        (
            "GOOGLE_CLIENT_ID",
            "legacy-client.apps.googleusercontent.com",
        ),
        ("GOOGLE_CLIENT_SECRET", "legacy-client-secret"),
        (
            "GOOGLE_OAUTH_REDIRECT_URI",
            "http://127.0.0.1:3000/legacy/callback",
        ),
        ("GOOGLE_ALLOWED_HD", "legacy.example.com"),
    ]);

    let config = resolve_google_oauth_config(|name| vars.get(name).map(|value| value.to_string()))
        .expect("Google OAuth config")
        .expect("configured Google OAuth");

    assert_eq!(
        config.client.client_id.as_str(),
        "reborn-client.apps.googleusercontent.com"
    );
    assert_eq!(
        config.client.redirect_uri.as_str(),
        "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback"
    );
    assert!(config.client.client_secret.is_some());
    assert_eq!(
        config.hosted_domain_hint.as_deref(),
        Some("reborn.example.com")
    );
}

#[test]
fn resolve_google_oauth_config_uses_legacy_client_vars_as_configuration_signal() {
    let vars = HashMap::from([
        (
            "GOOGLE_CLIENT_ID",
            "legacy-client.apps.googleusercontent.com",
        ),
        ("GOOGLE_CLIENT_SECRET", "legacy-client-secret"),
    ]);

    let error = resolve_google_oauth_config(|name| vars.get(name).map(|value| value.to_string()))
        .expect_err("legacy client vars without redirect URI must not be ignored");

    assert!(error.to_string().contains("GOOGLE_OAUTH_REDIRECT_URI"));
}
