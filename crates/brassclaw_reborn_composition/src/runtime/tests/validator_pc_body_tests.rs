//! Step 7-B: negative fixture tests for each of the 8 validator class PC bodies.
//!
//! Each test feeds a minimal valid candidate → asserts `pass: true` and an
//! invalid candidate with a class-specific defect → asserts `pass: false` with
//! a non-empty errors list. Bodies are called through the real Monty utility
//! worker (requires `installed_worker()` / `monty_worker` binary).
//!
//! All PC bodies receive `inputs["subject_json"]` — a JSON string encoding a
//! minimal component-validator-subject/1 envelope. The candidate document lives
//! at `subject["candidate"]["revision"]["document"]`.
//!
//! All PC bodies are pure-logic (only `import json`); no `host.*` calls.
//! See `missing4_v3_at_k3.md §Step 7-B` for the acceptance contract.
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{UtilityLimits, UtilityOutput, UtilityRequest, execute},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

// ---- PC body constants imported from builtin_bootstrap ----------------------

use crate::builtin_bootstrap::{
    PC_VALIDATOR_CLASS_0_BODY, PC_VALIDATOR_CLASS_13_BODY, PC_VALIDATOR_CLASS_21_BODY,
    PC_VALIDATOR_CLASS_22_BODY, PC_VALIDATOR_CLASS_23_BODY, PC_VALIDATOR_CLASS_SKILL_BODY,
};

// ---- Shared test helpers ----------------------------------------------------

/// Wrap a candidate document in a minimal component-validator-subject/1 envelope
/// and return `{"subject_json": <serialised envelope>}` as the inputs value.
///
/// The PC bodies parse `inputs["subject_json"]` and reach the document via
/// `subject["candidate"]["revision"]["document"]`.
fn make_subject_inputs(name: &str, description: &str, document: Value) -> Value {
    let sub_id = Uuid::new_v4();
    let envelope = json!({
        "format": "component-validator-subject/1",
        "submission_id": sub_id,
        "actor": "test",
        "subject_bytes": "{}",
        "subject_checksum": "0000000000000000000000000000000000000000000000000000000000000000",
        "candidate": {
            "reference": {
                "uuid": Uuid::new_v4(),
                "class_code": 22,
                "version": 1,
                "checksum": "0000000000000000000000000000000000000000000000000000000000000000"
            },
            "revision_bytes": "{}",
            "revision": {
                "document": {
                    "name": name,
                    "description": description,
                    // Merge the caller-supplied extra fields into the document.
                    // We do this by spreading the document fields in.
                    // serde_json doesn't have spread, so merge manually.
                }
            }
        },
        "dependencies": [],
        "base_comparison": null
    });
    // Merge caller-supplied document fields into the candidate revision document.
    let mut merged_doc = json!({
        "name": name,
        "description": description,
    });
    if let (Some(merged_obj), Some(doc_obj)) = (merged_doc.as_object_mut(), document.as_object()) {
        for (k, v) in doc_obj {
            merged_obj.insert(k.clone(), v.clone());
        }
    }
    let mut envelope_obj = envelope.as_object().unwrap().clone();
    if let Some(candidate) = envelope_obj.get_mut("candidate")
        && let Some(revision) = candidate.get_mut("revision")
        && let Some(obj) = revision.as_object_mut()
    {
        obj.insert("document".into(), merged_doc);
    }
    let subject_str = serde_json::Value::Object(envelope_obj).to_string();
    json!({"subject_json": subject_str})
}

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

/// Assert a body call produces `{rule_id: "structural_presence", ok: true, errors: []}`.
async fn assert_pass(body: &str, inputs: Value) {
    let out = run_body(body, inputs).await;
    assert_eq!(
        out["rule_id"],
        json!("structural_presence"),
        "expected rule_id=structural_presence, got: {out}"
    );
    assert_eq!(out["ok"], json!(true), "expected ok=true, got: {out}");
    assert_eq!(
        out["errors"],
        json!([]),
        "expected empty errors, got: {out}"
    );
}

/// Assert a body call produces `{rule_id: "structural_presence", ok: false, errors: [<non-empty>]}`.
async fn assert_fail(body: &str, inputs: Value) {
    let out = run_body(body, inputs).await;
    assert_eq!(
        out["rule_id"],
        json!("structural_presence"),
        "expected rule_id=structural_presence, got: {out}"
    );
    assert_eq!(out["ok"], json!(false), "expected ok=false, got: {out}");
    let errors = out["errors"].as_array().expect("errors must be array");
    assert!(
        !errors.is_empty(),
        "expected at least one error, got empty list; full output: {out}"
    );
}

// ---- Class 0 (Tool) ---------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_0_valid_candidate_passes() {
    assert_pass(
        PC_VALIDATOR_CLASS_0_BODY,
        make_subject_inputs(
            "tool-read-file",
            "Reads a file from disk.",
            json!({
                "capability_id": "builtin.read_file",
                "effect_type": "read",
                "param_schema": {"type": "object"}
            }),
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_0_missing_capability_id_fails() {
    assert_fail(
        PC_VALIDATOR_CLASS_0_BODY,
        make_subject_inputs(
            "tool-read-file",
            "Reads a file.",
            json!({
                "effect_type": "read",
                "param_schema": {"type": "object"}
            }),
        ),
    )
    .await;
}

// ---- Class 1–3 (Skill) ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_skill_valid_candidate_passes() {
    assert_pass(
        PC_VALIDATOR_CLASS_SKILL_BODY,
        make_subject_inputs(
            "skill-read-file",
            "Reads a file via host.read_file.",
            json!({
                "body": "def usage_read(inputs):\n    pass\n"
            }),
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_skill_missing_body_fails() {
    assert_fail(
        PC_VALIDATOR_CLASS_SKILL_BODY,
        make_subject_inputs("skill-read-file", "Reads a file.", json!({})),
    )
    .await;
}

// ---- Class 13 (ToolSkill) ---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_13_valid_candidate_passes() {
    assert_pass(
        PC_VALIDATOR_CLASS_13_BODY,
        make_subject_inputs(
            "ts-read-file",
            "ToolSkill for read_file.",
            json!({
                "content": "Binds the read_file tool."
            }),
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_13_missing_content_fails() {
    assert_fail(
        PC_VALIDATOR_CLASS_13_BODY,
        make_subject_inputs("ts-read-file", "ToolSkill for read_file.", json!({})),
    )
    .await;
}

// ---- Class 21 (Recipe) ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_21_valid_candidate_passes() {
    assert_pass(
        PC_VALIDATOR_CLASS_21_BODY,
        make_subject_inputs(
            "list-triggers",
            "Lists all configured triggers.",
            json!({
                "step_descriptions": [{"step_id": "step-1", "type": "component"}]
            }),
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_21_empty_step_descriptions_fails() {
    assert_fail(
        PC_VALIDATOR_CLASS_21_BODY,
        make_subject_inputs(
            "list-triggers",
            "Lists all configured triggers.",
            json!({
                "step_descriptions": []
            }),
        ),
    )
    .await;
}

// ---- Class 22 (PythonCode) --------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_22_valid_candidate_passes() {
    assert_pass(
        PC_VALIDATOR_CLASS_22_BODY,
        make_subject_inputs(
            "pc-read-file",
            "PythonCode for read_file.",
            json!({
                "content": "def usage_read(inputs):\n    return host.read_file(path=inputs['path'])\n"
            }),
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_22_missing_def_keyword_fails() {
    assert_fail(
        PC_VALIDATOR_CLASS_22_BODY,
        make_subject_inputs(
            "pc-bad",
            "PythonCode without def.",
            json!({
                "content": "result = host.read_file(path='x')\n"
            }),
        ),
    )
    .await;
}

// ---- Class 23 (ExtensionCatalogue) ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_23_valid_candidate_passes() {
    assert_pass(
        PC_VALIDATOR_CLASS_23_BODY,
        make_subject_inputs(
            "ext-filesystem",
            "Filesystem capability extension catalogue.",
            json!({
                "overview_doc": "Filesystem tools: read, write, list, glob, grep."
            }),
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn class_23_missing_overview_doc_fails() {
    assert_fail(
        PC_VALIDATOR_CLASS_23_BODY,
        make_subject_inputs("ext-filesystem", "Filesystem catalogue.", json!({})),
    )
    .await;
}
