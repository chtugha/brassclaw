//! Original configuration is data custody, never a caller's qualification grant.
use super::*;
use crate::installed_monty_catalogue::{PreparationCapture, PreparationOrigin};
use brassclaw_product_workflow::{MontyVmSettings, default_monty_vm_settings};

#[cfg(test)]
fn manifest(settings: &MontyVmSettings) -> Value {
    let executor =
        crate::global_monty_startup::ControlledExecutorConfiguration::new(settings).unwrap();
    let preparation = PreparationCapture::from_original(
        brassclaw_host_api::task_preparation::RetainedPreparation::encode(
            settings.revision,
            PreparationOrigin::AcknowledgedStartup,
            settings.execution_limits,
        )
        .unwrap(),
    )
    .unwrap();
    json!({"format":"original-pure-seed-manifest/1",
        "program":"exact retained program","worker_checksum":"original worker",
        "application_checksum":"original application","specs":["return","exception"],
        "executor_bytes":executor.exact_bytes(),"executor_checksum":executor.checksum(),
        "preparation_bytes":preparation.bytes()})
}

#[test]
fn native_case_original_configuration_survives_live_settings_edits() {
    let mut settings = default_monty_vm_settings();
    settings.revision = 1;
    let original = manifest(&settings);
    settings.revision += 1;
    settings.execution_limits.max_pending_host_calls += 1;
    let current = manifest(&settings);
    assert_ne!(original, current);
    let (executor, preparation) = original_configuration(&current, &original).unwrap();
    assert_eq!(json!(executor.exact_bytes()), original["executor_bytes"]);
    assert_eq!(json!(preparation.bytes()), original["preparation_bytes"]);
    assert_ne!(preparation.policy(), settings.execution_limits);
    assert_ne!(preparation.revision(), settings.revision);
}

#[test]
fn native_case_original_configuration_rejects_changed_package_and_corrupt_captures() {
    let mut settings = default_monty_vm_settings();
    settings.revision = 1;
    let original = manifest(&settings);
    for field in [
        "program",
        "worker_checksum",
        "application_checksum",
        "specs",
    ] {
        let mut changed = original.clone();
        changed[field] = json!("changed immutable original");
        assert!(
            original_configuration(&original, &changed).is_err(),
            "{field}"
        );
    }
    let mut changed = original.clone();
    changed["unexpected"] = json!(true);
    assert!(original_configuration(&original, &changed).is_err());
    for field in ["executor_bytes", "executor_checksum", "preparation_bytes"] {
        let mut missing = original.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            original_configuration(&original, &missing).is_err(),
            "{field}"
        );
        let mut corrupt = original.clone();
        corrupt[field] = json!("not a canonical original");
        assert!(
            original_configuration(&original, &corrupt).is_err(),
            "{field}"
        );
    }
}

#[test]
fn native_case_original_configuration_rejects_mixed_executor_and_preparation() {
    let mut settings = default_monty_vm_settings();
    settings.revision = 1;
    let original = manifest(&settings);
    settings.revision += 1;
    let later = manifest(&settings);
    let mut mixed = original.clone();
    mixed["preparation_bytes"] = later["preparation_bytes"].clone();
    assert!(original_configuration(&later, &mixed).is_err());
}
