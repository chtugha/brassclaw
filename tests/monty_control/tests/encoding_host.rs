//! Real retained utility-worker encoding; no global VM/provider is created.
use brassclaw_monty_host::{
    VmFailure,
    process::ProcessFailure,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

#[tokio::test]
async fn contained_model_encoding_matches_offline_oracle_and_rejects_bad_input() {
    let worker = PathBuf::from(
        std::env::var_os("BRASSCLAW_MONTY_WORKER_PATH").expect("retained worker required"),
    );
    let originals = PathBuf::from(
        std::env::var_os("BRASSCLAW_ENCODING_MODEL_ORIGINALS")
            .expect("retained model originals required"),
    );
    let read = |name| std::fs::read_to_string(originals.join(name)).unwrap();
    let request = json!({"format":"brassclaw-model-encoding-worker/1","assets":{"tokenizer_json":read("tokenizer.json"),"tokenizer_config_json":read("tokenizer_config.json"),"chat_template":read("chat_template.jinja")},"probes":[
        {"kind":"chat","context_json":json!({"messages":[{"role":"system","content":"Pinned reference material.\n".repeat(1500)},{"role":"user","content":"Inspect the firewall"}],"add_generation_prompt":true,"enable_thinking":false}).to_string()},
        {"kind":"chat","context_json":json!({"messages":[{"role":"system","content":"Pinned reference material.\n".repeat(1500)},{"role":"user","content":"Inspect the router"}],"add_generation_prompt":true,"enable_thinking":false}).to_string()}
    ]});
    let request_bytes = serde_json::to_string(&request).unwrap();
    drop(request);
    // Explicit acceptance-fixture capacity, never an implicit production override.
    let settings = brassclaw_host_api::MontyExecutionLimits {
        utility_worker_memory_bytes: 1024 * 1024 * 1024,
        response_timeout_millis: 120_000,
        ..Default::default()
    };
    let limits = UtilityLimits::from_execution(settings).unwrap();
    let output = utility::execute(
        &worker,
        UtilityRequest::EncodeModel {
            request_bytes: request_bytes.clone(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap();
    assert_eq!(
        format!("{output:?}"),
        "EncodedModel(<private encoding report>)"
    );
    let UtilityOutput::EncodedModel { report_bytes } = output else {
        panic!("actual encoding report required")
    };
    let actual: Value = serde_json::from_str(&report_bytes).unwrap();
    assert!(actual["probes"][0]["token_count"].as_u64().unwrap() > 1024);
    assert!(actual["common_prefix_tokens"].as_u64().unwrap() > 1024);
    let oracle =
        std::env::var_os("BRASSCLAW_TEST_ENCODING_ORACLE").expect("captured oracle required");
    let mut python = Command::new("python3")
        .arg(oracle)
        .env("HF_HUB_OFFLINE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    python
        .stdin
        .take()
        .unwrap()
        .write_all(request_bytes.as_bytes())
        .unwrap();
    let expected = python.wait_with_output().unwrap();
    assert!(expected.status.success());
    assert_eq!(
        actual,
        serde_json::from_slice::<Value>(&expected.stdout).unwrap()
    );
    let error = utility::execute(
        &worker,
        UtilityRequest::EncodeModel {
            request_bytes: "{}".into(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::InvalidInputs));
    assert!(error.exit_status.is_some_and(|status| status.success()));
    let mut invalid: Value = serde_json::from_str(&request_bytes).unwrap();
    invalid["assets"]["chat_template"] = json!("{{ private_request_field|unsupported_filter }}");
    let failure = utility::execute(
        &worker,
        UtilityRequest::EncodeModel {
            request_bytes: invalid.to_string(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap();
    let UtilityOutput::ModelEncodingFailed { failure } = failure else {
        panic!("classified template failure required")
    };
    assert_eq!(
        serde_json::to_value(failure).unwrap(),
        json!({"template":{"kind":"unknown_filter"}})
    );
    let mut short = limits.process;
    short.response_timeout = Duration::from_nanos(1);
    let timeout = utility::execute(
        &worker,
        UtilityRequest::EncodeModel {
            request_bytes,
            bounds: limits.bounds,
        },
        short,
    )
    .await
    .unwrap_err();
    assert_eq!(timeout.kind, ProcessFailure::Deadline);
    // Containment is attempted on timeout; no successful result is manufactured.
    assert!(timeout.exit_status.is_some() || timeout.reap_error.is_some());
}
