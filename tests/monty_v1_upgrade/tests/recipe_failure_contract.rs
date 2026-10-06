//! Check the actual Python sequencing source with deterministic host replies.
//! The source and host receiver are unchanged; production kernel routing still
//! requires caller-level acceptance beyond this interpreter sequencing check.

use brassclaw_monty_v1_upgrade_tests::namespace::{HOST_INSTANCE_ID, host_namespace};

use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExtFunctionResult, MontyException, MontyObject, PrintWriter, ResourceLimits,
    ResourceTracker,
};
use std::time::Duration;

fn object(fields: Vec<(&str, MontyObject)>) -> MontyObject {
    MontyObject::dict(
        fields
            .into_iter()
            .map(|(key, value)| (MontyObject::string(key), value)),
    )
}

struct Case {
    status: &'static str,
    missing_recipe: bool,
    failed_step: bool,
    failed_composition: bool,
}

fn drive(case: Case) -> (Vec<String>, Result<(), MontyException>) {
    let source = include_str!("../../../crates/brassclaw_engine/orchestrator/basic_mode.py");
    let code =
        format!("context = []\ngoal = ''\nactions = []\nstate = {{}}\nconfig = {{}}\n{source}");
    let run = MontyRun::new(
        code,
        "recipe-contract.py",
        vec!["host".to_owned()],
        CompileOptions::default(),
    )
    .unwrap();
    let mut progress = run
        .start(
            vec![host_namespace()],
            ResourceTracker::new(
                ResourceLimits::default()
                    .max_feed_duration(Duration::from_secs(5))
                    .max_suspensions(64),
            ),
            PrintWriter::Disabled,
        )
        .unwrap();
    let mut calls = Vec::new();
    let mut admitted = false;
    let mut steps = 0;
    for _ in 0..32 {
        let RunProgress::FunctionCall(call) = progress else {
            panic!("unexpected progress")
        };
        assert_eq!(call.object_id, Some(HOST_INSTANCE_ID));
        if call.function_name == "resolve_intent" {
            let (key, value) = call.args.kwargs().next().expect("user_input keyword");
            assert_eq!(key.to_owned(), MontyObject::string("user_input"));
            assert_eq!(value.to_owned(), MontyObject::string("the accepted input"));
            assert_eq!(call.args.args().len(), 0, "receiver is not an argument");
        }
        calls.push(call.function_name.clone());
        let answer = match call.function_name.as_str() {
            "check_signals" => MontyObject::none(),
            "await_next_turn" => {
                if admitted {
                    return (calls, Ok(()));
                }
                admitted = true;
                MontyObject::string("the accepted input")
            }
            "resolve_intent" => object(vec![
                ("status", MontyObject::string(case.status)),
                ("component_id", MontyObject::string("matched-recipe")),
                ("step_link", MontyObject::string("0:1-0:E")),
            ]),
            "resolve_component_by_name" => {
                if case.missing_recipe {
                    MontyObject::none()
                } else {
                    object(vec![("id", MontyObject::string("instruction"))])
                }
            }
            "compose_orchestrator" => object(vec![
                ("ok", MontyObject::bool(!case.failed_composition)),
                (
                    "program",
                    object(vec![(
                        "steplist",
                        MontyObject::list(vec![
                            object(vec![("executable_code", MontyObject::string("effect-one"))]),
                            object(vec![("executable_code", MontyObject::string("effect-two"))]),
                        ]),
                    )]),
                ),
            ]),
            "run_program" => {
                assert_eq!(call.args.args().len(), 2);
                let state = call.args.args().nth(1).unwrap();
                let fields = state.pairs().expect("Recipe state is a typed dictionary");
                let previous = fields
                    .iter()
                    .find(|(key, _)| key.as_str() == Some("previous_result"))
                    .expect("Monty owns the preceding result")
                    .1
                    .to_owned();
                assert_eq!(
                    previous,
                    if steps % 2 == 0 {
                        MontyObject::none()
                    } else {
                        MontyObject::string("answer")
                    }
                );
                steps += 1;
                object(vec![
                    ("ok", MontyObject::bool(!(case.failed_step && steps == 2))),
                    ("return_value", MontyObject::string("answer")),
                ])
            }
            "post_reply" => MontyObject::none(),
            _ => panic!("unexpected operation; no hidden LLM fallback is allowed"),
        };
        progress = match call.resume(ExtFunctionResult::Return(answer), PrintWriter::Disabled) {
            Ok(next) => next,
            Err(error) => return (calls, Err(error)),
        };
    }
    panic!("sequencing exceeded its bounded host-call budget")
}

fn case(status: &'static str) -> Case {
    Case {
        status,
        missing_recipe: false,
        failed_step: false,
        failed_composition: false,
    }
}

#[test]
fn matching_errors_unknown_status_and_ambiguity_never_start_tier_two() {
    for status in ["error", "unknown", "disambiguation"] {
        let (calls, result) = drive(case(status));
        let error = result.unwrap_err();
        assert!(error.to_string().contains(if status == "disambiguation" {
            "intent_disambiguation_required"
        } else {
            "intent_resolution_failed"
        }));
        assert!(!calls.iter().any(|call| call == "compose_orchestrator"
            || call == "resolve_component_by_name"
            || call == "run_program"));
    }
}

#[test]
fn failed_second_recipe_step_does_not_replay_the_first_effect_as_tier_two() {
    let (calls, result) = drive(Case {
        failed_step: true,
        ..case("match")
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("recipe_execution_failed")
    );
    assert_eq!(
        calls.iter().filter(|call| *call == "run_program").count(),
        2
    );
    assert!(
        !calls
            .iter()
            .any(|call| call == "resolve_component_by_name" || call == "post_reply")
    );
}

#[test]
fn composition_failure_does_not_start_another_recipe_or_llm_path() {
    let (calls, result) = drive(Case {
        failed_composition: true,
        ..case("match")
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("recipe_composition_failed")
    );
    assert!(
        !calls
            .iter()
            .any(|call| call == "resolve_component_by_name" || call == "run_program")
    );
}

#[test]
fn missing_no_match_instruction_does_not_call_the_llm_directly() {
    let (calls, result) = drive(Case {
        missing_recipe: true,
        ..case("no_match")
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("non_match_instruction_unavailable")
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| *call == "resolve_component_by_name")
            .count(),
        1
    );
    assert!(!calls.iter().any(|call| call == "run_program"));
}

#[test]
fn failed_no_match_instruction_does_not_call_the_llm_again() {
    let (calls, result) = drive(Case {
        failed_step: true,
        ..case("no_match")
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("recipe_execution_failed")
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| *call == "compose_orchestrator")
            .count(),
        1
    );
    assert_eq!(
        calls.iter().filter(|call| *call == "run_program").count(),
        2
    );
}

#[test]
fn genuine_no_match_uses_its_instruction_and_returns_to_the_work_wait() {
    let (calls, result) = drive(case("no_match"));
    result.unwrap();
    assert_eq!(calls.iter().filter(|call| *call == "post_reply").count(), 1);
    assert_eq!(
        calls
            .iter()
            .filter(|call| *call == "await_next_turn")
            .count(),
        2
    );
}
