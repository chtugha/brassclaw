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
use std::{collections::BTreeMap, path::Path};
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
    task_settings: brassclaw_monty_host::process::TaskSettings,
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
    append_observed_invocation(&mut source, &export);
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
                max_compute_time: task_settings.max_compute_time,
            },
            limits.process,
        )
        .await;
        // Only a Python ValueError caught inside the actual invocation is a
        // rejected protocol input. Containment, timeout, transport, syntax and
        // unrelated runtime failures remain infrastructure errors.
        let observation = match output.map_err(invalid)? {
            UtilityOutput::Evaluated { value, stdout } => {
                if !stdout.is_empty() {
                    return Err(invalid("free-workflow oracle produced unexpected stdout"));
                }
                if expect_success {
                    result_contract
                        .validate(&value["result"])
                        .map_err(invalid)?;
                }
                verify_case_observation(case_name, &value)?;
                value
            }
            other => {
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
    Ok(json!({"format": "free-workflow-pure-oracle/2", "cases": cases}))
}

/// Verify stored oracle observations match the independent fixture oracle.
pub(super) fn verify_oracle_observations(observations: &Value) -> Result<(), RebornBuildError> {
    if observations
        .as_object()
        .is_none_or(|object| object.len() != 2)
        || observations["format"] != "free-workflow-pure-oracle/2"
    {
        return Err(invalid("free-workflow oracle envelope differs"));
    }
    let cases = observations["cases"]
        .as_array()
        .filter(|v| v.len() == CASES.len())
        .ok_or_else(|| invalid("free-workflow oracle cases incomplete"))?;
    for (record, &case_name) in cases.iter().zip(CASES) {
        let (packet_bytes, answer_bytes, expect_success) = case_inputs(case_name)?;
        if record.as_object().is_none_or(|object| object.len() != 5)
            || record["case"] != case_name
            || record["packet_bytes_sha256"] != crate::bootstrap_evidence::checksum(&packet_bytes)
            || record["answer_bytes_sha256"] != crate::bootstrap_evidence::checksum(&answer_bytes)
            || record["expected_success"] != expect_success
        {
            return Err(invalid("free-workflow oracle case provenance differs"));
        }
        verify_case_observation(case_name, &record["observation"])?;
    }
    Ok(())
}

fn verify_case_observation(case: &str, observed: &Value) -> Result<(), RebornBuildError> {
    let (packet_bytes, _, expect_success) = case_inputs(case)?;
    let expected = if expect_success {
        let packet: Value = serde_json::from_str(&packet_bytes).map_err(invalid)?;
        json!({"succeeded":true,"result":canonical(&valid_answer(&packet))?})
    } else {
        let message = match case {
            "b1-wrong-request" | "b2-wrong-digest" | "b1-wrong-task" => {
                "bootstrap answer contract or echo mismatch"
            }
            "b2-extra-field" | "b1-wrong-body" => "bootstrap object fields differ",
            "b2-foreign-candidate" => "unknown bootstrap candidate",
            _ => return Err(invalid("negative oracle case has no declared rejection")),
        };
        json!({"succeeded":false,"rejection":{"kind":"ValueError","message":message}})
    };
    if observed != &expected {
        return Err(invalid(
            "free-workflow oracle actual outcome differs from the independent case",
        ));
    }
    Ok(())
}

/// Installation-only observation wrapper. Source comes from the inspected
/// preload graph; request values remain separately bound typed inputs.
fn append_observed_invocation(source: &mut String, export: &str) {
    source.push_str("try:\n");
    for line in export.lines() {
        source.push_str("    ");
        source.push_str(line);
        source.push('\n');
    }
    source.push_str("except ValueError as _oracle_error:\n    _oracle_observation = {'succeeded': False, 'rejection': {'kind': 'ValueError', 'message': str(_oracle_error)}}\nelse:\n    _oracle_observation = {'succeeded': True, 'result': result}\n_oracle_observation\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_packaged_protocol_oracle_observes_every_declared_case() {
        let worker = brassclaw_monty_host::process::installed_worker().unwrap();
        let limits = UtilityLimits::from_execution(Default::default()).unwrap();
        // Real packaged pure functions and real native execution. This checks
        // the finite oracle contract, not model/Tool or workflow qualification.
        let mut source = format!(
            "{}\n{}\n",
            brassclaw_skills::prefix_bootstrap_components::ANSWER_SOURCE,
            brassclaw_skills::bootstrap_answer_workflow::BOOTSTRAP_PROTOCOL_SOURCE,
        );
        append_observed_invocation(
            &mut source,
            "result = validate_captured_bootstrap_answer(inputs=inputs)",
        );
        for &case in CASES {
            let (packet_bytes, answer_bytes, _) = case_inputs(case).unwrap();
            let output = execute(
                &worker,
                UtilityRequest::Evaluate {
                    source: source.clone(),
                    inputs: BTreeMap::from([(
                        "inputs".into(),
                        json!({"packet_bytes":packet_bytes,"answer_bytes":answer_bytes}),
                    )]),
                    bounds: limits.bounds,
                    max_compute_time: std::time::Duration::from_secs(1),
                },
                limits.process,
            )
            .await
            .unwrap();
            let UtilityOutput::Evaluated { value, stdout } = output else {
                panic!("native packaged oracle returned the wrong operation output")
            };
            assert!(stdout.is_empty());
            verify_case_observation(case, &value).unwrap();
        }
    }

    #[tokio::test]
    async fn native_oracle_only_classifies_the_expected_protocol_rejection() {
        let worker = brassclaw_monty_host::process::installed_worker().unwrap();
        let limits = UtilityLimits::from_execution(Default::default()).unwrap();
        for (exception, rejected) in [
            ("ValueError", true),
            ("RuntimeError", false),
            ("MemoryError", false),
        ] {
            let mut source =
                format!("def oracle(inputs):\n    raise {exception}('fixture failure')\n");
            append_observed_invocation(&mut source, "result = oracle(inputs=inputs)");
            let result = execute(
                &worker,
                UtilityRequest::Evaluate {
                    source,
                    inputs: BTreeMap::from([("inputs".into(), json!({}))]),
                    bounds: limits.bounds,
                    max_compute_time: std::time::Duration::from_secs(1),
                },
                limits.process,
            )
            .await;
            if rejected {
                let UtilityOutput::Evaluated { value, stdout } = result.unwrap() else {
                    panic!("native oracle returned the wrong operation output")
                };
                assert!(stdout.is_empty());
                assert_eq!(
                    value,
                    json!({"succeeded":false,"rejection":{"kind":"ValueError","message":"fixture failure"}})
                );
            } else {
                assert!(
                    result.is_err(),
                    "unexpected {exception} cannot qualify a negative case"
                );
            }
        }
    }

    #[test]
    fn stored_oracle_requires_the_exact_independent_answer_and_rejection() {
        let (bytes, _, _) = case_inputs("b1-valid").unwrap();
        let packet: Value = serde_json::from_str(&bytes).unwrap();
        let actual = json!({"succeeded":true,"result":canonical(&valid_answer(&packet)).unwrap()});
        verify_case_observation("b1-valid", &actual).unwrap();
        assert!(
            verify_case_observation("b1-valid", &json!({"succeeded":true,"result":"unrelated"}))
                .is_err()
        );
        assert!(verify_case_observation("b1-wrong-request", &json!({"succeeded":false})).is_err());
        assert!(verify_case_observation("b1-wrong-request", &json!({
            "succeeded":false,"rejection":{"kind":"ValueError","message":"unrelated failure"}
        })).is_err());
        assert!(
            verify_case_observation(
                "b1-wrong-request",
                &json!({"succeeded":false,"rejection":"RuntimeError"})
            )
            .is_err()
        );
    }
}
