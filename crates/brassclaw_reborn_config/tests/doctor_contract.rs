use brassclaw_reborn_config::{RebornBootConfig, RebornDoctorReport};

#[test]
fn doctor_report_is_side_effect_free_and_states_v1_is_not_used() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = RebornBootConfig::resolve_from_env_parts(
        Some(temp.path().join("reborn-home").into_os_string()),
        None,
        None,
    )
    .expect("boot config should resolve");

    let report = RebornDoctorReport::from_config(config);

    assert_eq!(report.home_source_label(), "BRASSCLAW_REBORN_HOME");
    assert_eq!(report.v1_state(), "not-used");
    assert!(!report.home_path().exists());
}
