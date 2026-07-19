#![cfg(feature = "root-llm-provider")]

use brassclaw_reborn_composition::{RebornProviderAdmin, RebornProviderAdminError, RebornV1State};
use brassclaw_reborn_config::{RebornBootConfig, RebornHome, RebornProfile};

fn admin_for_home(reborn_home: &std::path::Path) -> RebornProviderAdmin {
    let home = RebornHome::resolve_from_env_parts(
        Some(reborn_home.as_os_str().to_os_string()),
        None,
        None,
    )
    .expect("valid reborn home");
    RebornProviderAdmin::new(RebornBootConfig::new(home, RebornProfile::LocalDev))
}

#[test]
fn list_unknown_provider_returns_known_provider_context() {
    let temp = tempfile::tempdir().expect("tempdir");
    let admin = admin_for_home(&temp.path().join("reborn-home"));

    let err = admin
        .list(Some("missing-provider"), false)
        .expect_err("unknown provider should reject");

    let RebornProviderAdminError::UnknownProvider {
        provider, known, ..
    } = err
    else {
        panic!("expected unknown provider error");
    };
    assert_eq!(provider, "missing-provider");
    assert!(known.contains(&"openai".to_string()), "known: {known:?}");
}

#[test]
fn provider_admin_json_omits_absolute_host_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let admin = admin_for_home(&temp.path().join("reborn-home"));

    let list_json = serde_json::to_value(admin.list(None, false).expect("list")).expect("json");
    assert!(list_json.get("config_file").is_none(), "json: {list_json}");
    assert!(
        list_json.get("providers_file").is_none(),
        "json: {list_json}"
    );
    assert_eq!(list_json["v1_state"], RebornV1State::NotUsed.as_str());

    let status_json = serde_json::to_value(admin.status().expect("status")).expect("json");
    assert!(
        status_json.get("config_file").is_none(),
        "json: {status_json}"
    );
    assert!(
        status_json.get("providers_file").is_none(),
        "json: {status_json}"
    );
    assert_eq!(status_json["routes"], "not-configured");
    assert_eq!(status_json["v1_state"], RebornV1State::NotUsed.as_str());
}
