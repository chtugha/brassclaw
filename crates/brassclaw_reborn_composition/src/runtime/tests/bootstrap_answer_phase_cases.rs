//! Missing public-export phase observations on the SAME inspected candidate.
//! All source/observation labels here are fixture DATA, not independent native
//! provenance, provider/Serving facts, prefill permission or successful Q1.
use super::*;

fn prefill_fixture() -> Value {
    let values = json!({"max_prefix_tokens":1024,"max_prompt_tokens":2048,
        "reserved_output_tokens":1024,"additional_framing_tokens":0});
    let existing = values.as_object().unwrap().keys().map(|field| {
        (field.clone(),json!({"value":null,"owner_revision":"fixture-original-fields","protection":"replaceable"}))
    }).collect::<serde_json::Map<String,Value>>();
    json!({"decision":{"method":"deterministic"},"policy_ref":"fixture-original-policy",
        "eligible_options":[{"candidate_id":"original-option","rule_ref":"fixture-rule","rule_revision":"fixture-rule-v1","values":values,
            "requires":[{"field":"context_window_tokens","scope":"endpoint_configuration","value":8192,"evidence_ref":"fixture-context"}]}],
        "checked_observations":[{"field":"context_window_tokens","scope":"endpoint_configuration","status":"independently_verified","value":8192,"evidence_ref":"fixture-context"}],
        "existing_values":existing})
}

fn model_choice_fixture(mut data: Value, chosen: Value) -> Value {
    // Reuse A's protocol fixtures. This is not a model call/response receipt.
    let packet = packet("select_bootstrap_prefill");
    let mut answer = answer(&packet);
    answer["result"]["candidate_id"] = chosen;
    data["decision"] = json!({"method":"model","protocol":{
        "answer":answer,"expected_task":packet["payload"]["task"],
        "expected_request_id":packet["request_id"],"expected_input_sha256":packet["input_sha256"],
        "required_facts":packet["payload"]["required_facts"],
        "eligible_candidate_ids":["original-option"]}});
    data
}

pub(super) async fn observe(
    candidate: &InspectedRetainedProgram,
    answer_stage: Uuid,
    worker: &std::path::Path,
) {
    // Existing pure_component already checked this exact closure is host-free;
    // existing evaluate_pure applies its retained typed contract and real worker.
    let scalar = json!({"fact":{"field":"context_window_tokens","scope":"endpoint_configuration","status":"proposed","value":32768,
        "evidence":[{"source_id":"fixture-source","locator":"/json/limit","quote":"32768"}]},
        "source_views":{"fixture-source":{"json":{"limit":32768}}}});
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"evidence","data_bytes":canonical(&scalar)}),
        worker,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(value.as_str().unwrap()).unwrap(),
        json!({"state":"native_scalar_match"})
    );

    let mut text = scalar.clone();
    text["fact"]["evidence"][0]["locator"] = json!("/text");
    text["source_views"]["fixture-source"] = json!({"text":"Documentation states 32768 tokens."});
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"evidence","data_bytes":canonical(&text)}),
        worker,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(value.as_str().unwrap()).unwrap(),
        json!({"state":"semantic_review_required"})
    );

    let mut conflict = scalar.clone();
    conflict["fact"]["status"] = json!("conflict");
    conflict["fact"]["value"] = Value::Null;
    conflict["fact"]["evidence"]
        .as_array_mut()
        .unwrap()
        .push(json!({"source_id":"fixture-source","locator":"/json/other","quote":"16384"}));
    conflict["source_views"]["fixture-source"]["json"]["other"] = json!(16384);
    for (other, quote, expected) in [
        (16384, "16384", "conflict"),
        (32768, "32768", "semantic_review_required"),
    ] {
        conflict["source_views"]["fixture-source"]["json"]["other"] = json!(other);
        conflict["fact"]["evidence"][1]["quote"] = json!(quote);
        let value = evaluate_pure(
            candidate,
            answer_stage,
            "bootstrap_answer_stage",
            json!({"phase":"evidence","data_bytes":canonical(&conflict)}),
            worker,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(value.as_str().unwrap()).unwrap(),
            json!({"state":expected})
        );
    }
    let mut unknown = scalar.clone();
    unknown["fact"]["status"] = json!("unknown");
    unknown["fact"]["value"] = Value::Null;
    unknown["fact"]["evidence"] = json!([]);
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"evidence","data_bytes":canonical(&unknown)}),
        worker,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(value.as_str().unwrap()).unwrap(),
        json!({"state":"unknown"})
    );
    for invalid in ["quote", "source"] {
        let mut data = scalar.clone();
        data["fact"]["evidence"][0][if invalid == "quote" {
            "quote"
        } else {
            "source_id"
        }] = json!("foreign");
        assert!(
            evaluate_pure(
                candidate,
                answer_stage,
                "bootstrap_answer_stage",
                json!({"phase":"evidence","data_bytes":canonical(&data)}),
                worker
            )
            .await
            .is_err(),
            "{invalid}"
        );
    }

    let data = prefill_fixture();
    let original = data.clone();
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"prefill","data_bytes":canonical(&data)}),
        worker,
    )
    .await
    .unwrap();
    let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(result["state"], "candidate_checked");
    assert_eq!(result["selection_method"], "deterministic");
    assert_eq!(result["model_answer_echo"], Value::Null);
    assert_eq!(result["policy_ref"], "fixture-original-policy");
    assert_eq!(result["rule_revision"], "fixture-rule-v1");
    assert!(
        result["field_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["field"] == "reserved_output_tokens"
                && v["value"] == 1024
                && v["expected_owner_revision"] == "fixture-original-fields")
    );
    assert_eq!(data, original);
    assert!(result.get("accepted").is_none());
    assert!(result.get("eligible_for_prefill").is_none());

    // Model selection is still fixture protocol DATA. None never picks the
    // first eligible option. Other source/interpreter/native checks stay open.
    let none = model_choice_fixture(prefill_fixture(), Value::Null);
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"prefill","data_bytes":canonical(&none)}),
        worker,
    )
    .await
    .unwrap();
    let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(result["state"], "no_candidate");
    assert_eq!(result["field_updates"], json!([]));
    assert_eq!(result["field_preconditions"], json!([]));
    for protected in [json!(0), Value::Null, json!(1024)] {
        let mut data = model_choice_fixture(prefill_fixture(), json!("original-option"));
        data["existing_values"]["max_prefix_tokens"]["value"] = protected.clone();
        data["existing_values"]["max_prefix_tokens"]["protection"] = json!("preserve");
        let value = evaluate_pure(
            candidate,
            answer_stage,
            "bootstrap_answer_stage",
            json!({"phase":"prefill","data_bytes":canonical(&data)}),
            worker,
        )
        .await
        .unwrap();
        let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
        if protected == json!(1024) {
            assert_eq!(result["state"], "candidate_checked");
            assert!(
                !result["field_updates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["field"] == "max_prefix_tokens")
            );
            assert!(result["field_preconditions"].as_array().unwrap().contains(&json!({"field":"max_prefix_tokens","value":1024,"owner_revision":"fixture-original-fields","protection":"preserve"})));
        } else {
            assert_eq!(result["state"], "protected_existing_value");
            assert_eq!(result["field_updates"], json!([]));
            assert_eq!(result["field_preconditions"], json!([]));
        }
    }
    // Exercise the existing transitive budget export through bootstrap's own
    // phase contract, not a second arithmetic implementation or new token cap.
    let mut disabled = model_choice_fixture(prefill_fixture(), json!("original-option"));
    disabled["eligible_options"][0]["values"]["max_prefix_tokens"] = json!(0);
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"prefill","data_bytes":canonical(&disabled)}),
        worker,
    )
    .await
    .unwrap();
    let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(result["state"], "candidate_checked");
    assert_eq!(result["budget"]["status"], "prefix-disabled");
    let mut oversized = model_choice_fixture(prefill_fixture(), json!("original-option"));
    oversized["eligible_options"][0]["values"]["reserved_output_tokens"] = json!(8192);
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"prefill","data_bytes":canonical(&oversized)}),
        worker,
    )
    .await
    .unwrap();
    let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(result["state"], "budget_not_fit");
    assert_eq!(result["budget"]["reserved_output_tokens"], 8192);
    assert_eq!(result["field_updates"], json!([]));
    let mut output_limit = model_choice_fixture(prefill_fixture(), json!("original-option"));
    output_limit["checked_observations"].as_array_mut().unwrap().push(json!({"field":"output_limit_tokens","scope":"endpoint_configuration","status":"independently_verified","value":512,"evidence_ref":"fixture-output"}));
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"prefill","data_bytes":canonical(&output_limit)}),
        worker,
    )
    .await
    .unwrap();
    let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(result["state"], "budget_not_fit");
    assert_eq!(result["budget"]["reserved_output_tokens"], 1024);
    assert_eq!(result["field_updates"], json!([]));

    for (ambiguous, expected) in [(false, "no_applicable_option"), (true, "ambiguous_options")] {
        let mut data = prefill_fixture();
        if ambiguous {
            let mut other = data["eligible_options"][0].clone();
            other["candidate_id"] = json!("another-option");
            data["eligible_options"].as_array_mut().unwrap().push(other);
        } else {
            data["eligible_options"] = json!([]);
        }
        let value = evaluate_pure(
            candidate,
            answer_stage,
            "bootstrap_answer_stage",
            json!({"phase":"prefill","data_bytes":canonical(&data)}),
            worker,
        )
        .await
        .unwrap();
        let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
        assert_eq!(result["state"], expected);
        assert_eq!(result["field_updates"], json!([]));
    }
    let mut wrong_reference = model_choice_fixture(prefill_fixture(), json!("original-option"));
    wrong_reference["eligible_options"][0]["requires"][0]["evidence_ref"] =
        json!("foreign-reference");
    let value = evaluate_pure(
        candidate,
        answer_stage,
        "bootstrap_answer_stage",
        json!({"phase":"prefill","data_bytes":canonical(&wrong_reference)}),
        worker,
    )
    .await
    .unwrap();
    let result: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(result["state"], "requirements_not_met");
    assert_eq!(result["field_updates"], json!([]));
    for missing in ["status", "scope"] {
        let mut data = prefill_fixture();
        if missing == "status" {
            data["checked_observations"][0]["status"] = json!("llm_proposed");
        } else {
            data["checked_observations"][0]["scope"] = json!("model_declared");
        }
        assert!(
            evaluate_pure(
                candidate,
                answer_stage,
                "bootstrap_answer_stage",
                json!({"phase":"prefill","data_bytes":canonical(&data)}),
                worker
            )
            .await
            .is_err(),
            "{missing}"
        );
    }
}
