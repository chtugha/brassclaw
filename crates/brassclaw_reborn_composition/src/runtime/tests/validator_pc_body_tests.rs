//! Step 7-B: negative fixture tests for each of the 8 validator class PC bodies.
//!
//! Each test feeds a minimal valid candidate → asserts `pass: true` and an
//! invalid candidate with a class-specific defect → asserts `pass: false` with
//! a non-empty errors list. Bodies are called through the real Monty utility
//! worker (requires `installed_worker()` / `monty_worker` binary).
//!
//! All PC bodies are pure-logic (no `host.*` calls); only `import json` is used.
//! See `missing4_v3_at_k3.md §Step 7-B` for the acceptance contract.
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{UtilityLimits, UtilityOutput, UtilityRequest, execute},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// ---- PC body constants imported from builtin_bootstrap ----------------------

use crate::builtin_bootstrap::{
    PC_VALIDATOR_CLASS_0_BODY, PC_VALIDATOR_CLASS_13_BODY, PC_VALIDATOR_CLASS_21_BODY,
    PC_VALIDATOR_CLASS_22_BODY, PC_VALIDATOR_CLASS_23_BODY, PC_VALIDATOR_CLASS_SKILL_BODY,
};

// ---- Shared test helpers ----------------------------------------------------

/// Build `UtilityRequest::Evaluate` that calls `check_structural_presence(inputs)`.
fn evaluate_body(body: &str, inputs: Value, limits: &UtilityLimits) -> UtilityRequest {
    let source = format!("{body}\ncheck_structural_presence(inputs)\n");
    UtilityRequest::Evaluate {
        source,
        inputs: BTreeMap::from([("inputs".into(), inputs)]),
        bounds: limits.bounds,
        max_compute_time: std::time::Duration::from_secs(5),
    }
}

/// Run a PC body with the given inputs and return the JSON output value.
async fn run_body(body: &str, inputs: Value) -> Value {
    let worker = installed_worker().expect("monty_worker binary must be present");
    let limits = UtilityLimits::from_execution(Default::default()).unwrap();
    let req = evaluate_body(body, inputs, &limits);
    match execute(&worker, req, limits.process).await.unwrap() {
        UtilityOutput::Evaluated { value, .. } => value,
        other => panic!("unexpected output: {other:?}"),
    }
}

/// Assert a body call produces `{pass: true, errors: []}`.
async fn assert_pass(body: &str, inputs: Value) {
    let out = run_body(body, inputs).await;
    assert_eq!(out["pass"], json!(true), "expected pass=true, got: {out}");
    assert_eq!(
        out["errors"],
        json!([]),
        "expected empty errors, got: {out}"
    );
}

/// Assert a body call produces `{pass: false, errors: [<non-empty>]}`.
async fn assert_fail(body: &str, inputs: Value) {
    let out = run_body(body, inputs).await;
    assert_eq!(out["pass"], json!(false), "expected pass=false, got: {out}");
    let errors = out["errors"].as_array().expect("errors must be array");
    assert!(
        !errors.is_empty(),
        "expected at least one error, got empty list; full output: {out}"
    );
}

// ---- Class 0 (Tool) ---------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_0_valid_candidate_passes() {
    let content = json!({
        "capability_id": "builtin.read_file",
        "effect_type": "read",
        "param_schema": {"type": "object"},
        "description": "Read a file from disk."
    });
    assert_pass(
        PC_VALIDATOR_CLASS_0_BODY,
        json!({"name": "tool-read-file", "description": "Reads a file.", "content": content.to_string()}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_0_missing_capability_id_fails() {
    let content = json!({
        "effect_type": "read",
        "param_schema": {"type": "object"},
        "description": "Read a file."
    });
    assert_fail(
        PC_VALIDATOR_CLASS_0_BODY,
        json!({"name": "tool-read-file", "description": "Reads a file.", "content": content.to_string()}),
    )
    .await;
}

// ---- Class 1–3 (Skill) ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_skill_valid_candidate_passes() {
    let content = json!({
        "body": "def usage_read(inputs):\n    pass\n",
        "class_code": 1,
        "description": "Reads a file via host.read_file."
    });
    assert_pass(
        PC_VALIDATOR_CLASS_SKILL_BODY,
        json!({"name": "skill-read-file", "description": "Reads a file.", "content": content.to_string()}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_skill_wrong_class_code_fails() {
    let content = json!({
        "body": "def usage_read(inputs):\n    pass\n",
        "class_code": 0,
        "description": "Not a Skill class."
    });
    assert_fail(
        PC_VALIDATOR_CLASS_SKILL_BODY,
        json!({"name": "skill-read-file", "description": "Reads a file.", "content": content.to_string()}),
    )
    .await;
}

// ---- Class 13 (ToolSkill) ---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_13_valid_candidate_passes() {
    let content = json!({
        "tool_name": "read_file",
        "content": "Binds the read_file tool.",
        "param_schema": {"type": "object"},
        "description": "IBS binding for read_file."
    });
    assert_pass(
        PC_VALIDATOR_CLASS_13_BODY,
        json!({"name": "ts-read-file", "description": "ToolSkill for read_file.", "content": content.to_string()}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_13_missing_tool_name_fails() {
    let content = json!({
        "content": "Binds some tool.",
        "param_schema": {"type": "object"},
        "description": "Missing tool_name."
    });
    assert_fail(
        PC_VALIDATOR_CLASS_13_BODY,
        json!({"name": "ts-read-file", "description": "ToolSkill for read_file.", "content": content.to_string()}),
    )
    .await;
}

// ---- Class 21 (Recipe) ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_21_valid_candidate_passes() {
    let content = json!({
        "name": "list-triggers",
        "description": "Lists all configured triggers.",
        "step_descriptions": [{"step_id": "step-1", "type": "component"}]
    });
    assert_pass(
        PC_VALIDATOR_CLASS_21_BODY,
        json!({"name": "list-triggers", "description": "Lists triggers.", "content": content.to_string()}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_21_empty_step_descriptions_fails() {
    let content = json!({
        "name": "list-triggers",
        "description": "Lists all configured triggers.",
        "step_descriptions": []
    });
    assert_fail(
        PC_VALIDATOR_CLASS_21_BODY,
        json!({"name": "list-triggers", "description": "Lists triggers.", "content": content.to_string()}),
    )
    .await;
}

// ---- Class 22 (PythonCode) --------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_22_valid_candidate_passes() {
    let content = json!({
        "content": "def usage_read(inputs):\n    return host.read_file(path=inputs['path'])\n",
        "description": "Reads a file via host.read_file."
    });
    assert_pass(
        PC_VALIDATOR_CLASS_22_BODY,
        json!({"name": "pc-read-file", "description": "PythonCode for read_file.", "content": content.to_string()}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_22_missing_def_keyword_fails() {
    let content = json!({
        "content": "result = host.read_file(path='x')\n",
        "description": "No def — not a valid preloadable function."
    });
    assert_fail(
        PC_VALIDATOR_CLASS_22_BODY,
        json!({"name": "pc-bad", "description": "PythonCode without def.", "content": content.to_string()}),
    )
    .await;
}

// ---- Class 23 (ExtensionCatalogue) ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_23_valid_candidate_passes() {
    let content = json!({
        "overview_doc": "Filesystem tools: read, write, list, glob, grep.",
        "description": "Filesystem capability extension catalogue."
    });
    assert_pass(
        PC_VALIDATOR_CLASS_23_BODY,
        json!({"name": "ext-filesystem", "description": "Filesystem catalogue.", "content": content.to_string()}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_23_missing_overview_doc_fails() {
    let content = json!({
        "description": "Missing overview_doc field."
    });
    assert_fail(
        PC_VALIDATOR_CLASS_23_BODY,
        json!({"name": "ext-filesystem", "description": "Filesystem catalogue.", "content": content.to_string()}),
    )
    .await;
}
