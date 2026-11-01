//! Independent finite oracle for the B1/B2 answer protocol's pure structural
//! components. This verifies exact graph assembly and protocol correctness using
//! the existing utility evaluator. No model Tool is invoked; the fixture does not
//! approve the workflow, activate a Free route or supply provider qualification.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::executor::retained_source::InspectedRetainedProgram;
use brassclaw_monty_host::utility::{UtilityLimits, UtilityOutput, UtilityRequest, execute};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Duration};
use uuid::Uuid;

/// Cases exercised by the contained pure oracle.
pub(super) const CASES: &[&str] = &[
    "b1-valid",
    "b2-valid",
    "b1-wrong-request",
    "b2-wrong-digest",
    "b1-wrong-task",
    "b2-extra-field",
    "b1-wrong-body",
    "b2-foreign-candidate",
];

fn packet(task: &str) -> Value {
    // Developer fixture DATA. These values are installation oracle constants;
    // they grant no native source, serving facts or dispatch qualification.
    json!({
        "request_id": "fixture-request-id",
        "input_sha256": "a".repeat(64),
        "payload": {
            "task": task,
            "target": {"specification": "b".repeat(64)},
            "executing_pair": {"specification": "c".repeat(64)},
            "required_facts": if task == "extract_bootstrap_facts" {
                json!([{"field": "context_window_tokens", "scope": "endpoint_configuration"}])
            } else {
                json!([])
            },
            "sources": [],
            "checked_observations": [],
            "eligible_options": if task == "select_bootstrap_prefill" {
                json!([{"candidate_id": "fixture-candidate"}])
            } else {
                json!([])
            },
            "existing_values": {},
            "response_contract": {
                "type": "object",
                "required": ["schema", "task", "request_id", "input_sha256", "result"],
                "additionalProperties": false,
                "properties": {
                    "schema": {"type": "string"},
                    "task": {"type": "string"},
                    "request_id": {"type": "string"},
                    "input_sha256": {"type": "string"},
                    "result": {"type": "object"}
                }
            }
        }
    })
}

fn valid_answer(pkt: &Value) -> Value {
    let task = pkt["payload"]["task"].as_str().unwrap_or("unknown");
    json!({
        "schema": "brassclaw-bootstrap-answer/1",
        "task": task,
        "request_id": pkt["request_id"],
        "input_sha256": pkt["input_sha256"],
        "result": if task == "extract_bootstrap_facts" {
            json!({"facts": [{"field": "context_window_tokens", "scope": "endpoint_configuration",
                "status": "unknown", "value": null, "evidence": []}], "issues": ["missing_evidence"]})
        } else {
            json!({"candidate_id": "fixture-candidate", "issues": []})
        }
    })
}

fn canonical(value: &Value) -> Result<String, RebornBuildError> {
    crate::prefix_generations::canonical(value).map_err(invalid)
}

/// Inputs for a given case: (packet_bytes, answer_bytes, expect_success).
pub(super) fn case_inputs(case: &str) -> Result<(String, String, bool), RebornBuildError> {
    let b1 = packet("extract_bootstrap_facts");
    let b2 = packet("select_bootstrap_prefill");
    let (pkt, answer, success): (&Value, Value, bool) = match case {
        "b1-valid" => {
            let a = valid_answer(&b1);
            (&b1, a, true)
        }
        "b2-valid" => {
            let a = valid_answer(&b2);
            (&b2, a, true)
        }
        "b1-wrong-request" => {
            let mut a = valid_answer(&b1);
            a["request_id"] = json!("foreign-request");
            (&b1, a, false)
        }
        "b2-wrong-digest" => {
            let mut a = valid_answer(&b2);
            a["input_sha256"] = json!("f".repeat(64));
            (&b2, a, false)
        }
        "b1-wrong-task" => {
            let mut a = valid_answer(&b1);
            a["task"] = json!("other_task");
            (&b1, a, false)
        }
        "b2-extra-field" => {
            let mut a = valid_answer(&b2);
            a["unexpected"] = json!(true);
            (&b2, a, false)
        }
        "b1-wrong-body" => {
            let mut a = valid_answer(&b1);
            a["result"] = json!("instructions are data");
            (&b1, a, false)
        }
        "b2-foreign-candidate" => {
            let mut a = valid_answer(&b2);
            a["result"]["candidate_id"] = json!("not-a-valid-option");
            (&b2, a, false)
        }
        _ => {
            return Err(invalid(format!(
                "unknown free-workflow fixture case: {case}"
            )));
        }
    };
    Ok((canonical(pkt)?, canonical(&answer)?, success))
}

/// Execute the pure `validate_captured_bootstrap_answer` oracle over all cases
/// and return structured observations. The result is not an approval record;
/// the approval owner (`bootstrap_free_workflow_validation.rs`) verifies it.
pub(super) async fn execute_pure_oracle(
    inspected: &InspectedRetainedProgram,
    protocol_id: Uuid,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<Value, RebornBuildError> {
    let pure = inspected.pure_component(protocol_id).map_err(invalid)?;
    let graph = inspected
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions();
    let doc = graph
        .get(&protocol_id)
        .ok_or_else(|| invalid("free-workflow protocol component absent from graph"))?
        .draft()
        .document();
    // Validate the input contract is well-formed before oracle execution.
    InputContract::from_value(&doc["input_contract"], REVISION_LIMITS).map_err(invalid)?;
    let result_contract =
        ValueContract::result_from_value(&doc["result_contract"], REVISION_LIMITS)
            .map_err(invalid)?;
    // Build source: preload closure then the export invocation.
    let mut source = String::new();
    for id in inspected.preload_order() {
        if pure.components().contains_key(id) {
            let component_source = graph
                .get(id)
                .ok_or_else(|| invalid("free-workflow preload component absent"))?
                .draft()
                .document()["content"]
                .as_str()
                .ok_or_else(|| invalid("free-workflow preload source missing"))?;
            source.push_str(component_source);
            source.push('\n');
        }
    }
    let export = pure
        .export_invocations()
        .get("validate_captured_bootstrap_answer")
        .ok_or_else(|| invalid("free-workflow protocol export absent"))?
        .clone();
    source.push_str(&export);
    source.push_str("\nresult\n");
    let mut cases = Vec::new();
    for &case_name in CASES {
        let (packet_bytes, answer_bytes, expect_success) = case_inputs(case_name)?;
        let inputs = BTreeMap::from([(
            "inputs".into(),
            json!({"packet_bytes": packet_bytes, "answer_bytes": answer_bytes}),
        )]);
        let output = execute(
            worker,
            UtilityRequest::Evaluate {
                source: source.clone(),
                inputs,
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await;
        let observation = match output {
            Ok(UtilityOutput::Evaluated { value, stdout }) => {
                if !expect_success {
                    return Err(invalid(format!(
                        "free-workflow oracle expected failure for case {case_name}"
                    )));
                }
                if !stdout.is_empty() {
                    return Err(invalid("free-workflow oracle produced unexpected stdout"));
                }
                result_contract.validate(&value).map_err(invalid)?;
                json!({"succeeded": true, "result": value})
            }
            Err(_) => {
                if expect_success {
                    return Err(invalid(format!(
                        "free-workflow oracle expected success for case {case_name}"
                    )));
                }
                json!({"succeeded": false})
            }
            Ok(other) => {
                return Err(invalid(format!(
                    "free-workflow oracle unexpected output kind for {case_name}: {other:?}"
                )));
            }
        };
        cases.push(json!({
            "case": case_name,
            "packet_bytes_sha256": crate::bootstrap_evidence::checksum(&packet_bytes),
            "answer_bytes_sha256": crate::bootstrap_evidence::checksum(&answer_bytes),
            "expected_success": expect_success,
            "observation": observation
        }));
    }
    Ok(json!({"format": "free-workflow-pure-oracle/1", "cases": cases}))
}

/// Verify stored oracle observations match the independent fixture oracle.
pub(super) fn verify_oracle_observations(observations: &Value) -> Result<(), RebornBuildError> {
    let cases = observations["cases"]
        .as_array()
        .filter(|v| v.len() == CASES.len())
        .ok_or_else(|| invalid("free-workflow oracle cases incomplete"))?;
    for (record, &case_name) in cases.iter().zip(CASES) {
        let (packet_bytes, answer_bytes, expect_success) = case_inputs(case_name)?;
        if record["case"] != case_name
            || record["packet_bytes_sha256"] != crate::bootstrap_evidence::checksum(&packet_bytes)
            || record["answer_bytes_sha256"] != crate::bootstrap_evidence::checksum(&answer_bytes)
            || record["expected_success"] != expect_success
        {
            return Err(invalid("free-workflow oracle case provenance differs"));
        }
        let succeeded = record["observation"]["succeeded"]
            .as_bool()
            .ok_or_else(|| invalid("free-workflow oracle observation malformed"))?;
        if succeeded != expect_success {
            return Err(invalid("free-workflow oracle success state differs"));
        }
        if expect_success && record["observation"]["result"].is_null() {
            return Err(invalid(
                "free-workflow oracle expected result missing for valid case",
            ));
        }
    }
    Ok(())
}
