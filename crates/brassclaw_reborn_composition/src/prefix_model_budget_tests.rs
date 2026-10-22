//! Actual Monty arithmetic and typed contracts; no policy resolution, generation
//! admission, selection, inference or rendered-partition qualification claimed.
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    value_contract::{InputContract, ValueContract},
};
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};

#[tokio::test]
async fn model_budget_planning_preserves_prompt_output_and_separate_policy_caps() {
    let draft =
        brassclaw_skills::prefix_compiler_components::model_budget_draft(uuid::Uuid::new_v4())
            .unwrap();
    let inputs =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    let result =
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
            .unwrap();
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let base = json!({"policy_ref":"provider/ornith/revision/7","max_prefix_tokens":100000,"max_prompt_tokens":14688,
        "context_window_tokens":131072,"reserved_output_tokens":16384,"additional_framing_tokens":0,"requested_prefix_tokens":120000});
    for (changes, expected, shortfall, status) in [
        (json!({}), 100000, 0, "planning-ready"),
        (
            json!({"context_window_tokens":131071}),
            99999,
            0,
            "planning-ready",
        ),
        (
            json!({"requested_prefix_tokens":60000}),
            60000,
            0,
            "planning-ready",
        ),
        (
            json!({"max_prefix_tokens":90000}),
            90000,
            0,
            "planning-ready",
        ),
        (
            json!({"additional_framing_tokens":1}),
            99999,
            0,
            "planning-ready",
        ),
        (
            json!({"context_window_tokens":31071}),
            0,
            1,
            "reservation-blocked",
        ),
        (json!({"max_prefix_tokens":0}), 0, 0, "prefix-disabled"),
    ] {
        let mut value = base.clone();
        value
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        inputs.bind(&value).unwrap();
        let UtilityOutput::Evaluated {
            value: answer,
            stdout,
        } = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: format!(
                    "{}\nplan_prefix_model_budget(inputs)\n",
                    draft.document()["content"].as_str().unwrap()
                ),
                inputs: BTreeMap::from([("inputs".into(), value.clone())]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap()
        else {
            panic!("pure budget answer required")
        };
        assert!(stdout.is_empty());
        result.validate(&answer).unwrap();
        assert_eq!(answer["effective_prefix_budget"], expected);
        assert_eq!(answer["reservation_shortfall_tokens"], shortfall);
        assert_eq!(answer["status"], status);
        assert_eq!(answer["policy_ref"], value["policy_ref"]);
        assert_eq!(answer["max_prompt_tokens"], 14688);
        assert_eq!(answer["reserved_output_tokens"], 16384);
        assert!(
            answer["qualification"]
                .as_str()
                .unwrap()
                .starts_with("planning-only")
        );
    }
    for (name, bad) in [
        ("max_prompt_tokens", json!(0)),
        ("max_prefix_tokens", json!(true)),
        ("reserved_output_tokens", json!(-1)),
        ("requested_prefix_tokens", json!(null)),
    ] {
        let mut value = base.clone();
        value[name] = bad;
        assert!(inputs.bind(&value).is_err());
    }
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("max_prefix_tokens");
    assert!(inputs.bind(&missing).is_err());
}
