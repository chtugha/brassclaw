// Profile contract tests removed in Phase 11 — `RebornProfile` and
// `BRASSCLAW_REBORN_PROFILE` were deleted. Boot configuration now only
// resolves home from the environment. Profile selection uses
// `BRASSCLAW_RUNTIME_PROFILE` (see `brassclaw_reborn_cli::runtime`).

use brassclaw_reborn_config::{RebornBootConfig, RebornHome};

#[test]
fn boot_config_resolves_home_from_env_parts() {
    let temp = tempfile::tempdir().expect("tempdir");

    let config = RebornBootConfig::resolve_from_env_parts(
        Some(temp.path().join("reborn-home").into_os_string()),
        None,
        None,
    )
    .expect("boot config should resolve");

    assert_eq!(
        config.home().path(),
        temp.path().join("reborn-home").as_path()
    );
}

#[test]
fn boot_config_resolves_home_default_from_env_parts() {
    let temp = tempfile::tempdir().expect("tempdir");

    let config =
        RebornBootConfig::resolve_from_env_parts(None, Some(temp.path().into()), None)
            .expect("boot config should resolve");

    let home: RebornHome = config.into_parts();
    assert!(home.path().starts_with(temp.path()));
}
