use brassclaw_skills::value_contract::{ContractLimits, InputContract, ValueContract, strict_json};
use serde_json::{Value, json};

fn limits() -> ContractLimits {
    ContractLimits {
        max_depth: 32,
        max_nodes: 4096,
        max_bytes: 65536,
    }
}

fn inputs(value: Value) -> InputContract {
    InputContract::from_value(&value, limits()).unwrap()
}
fn output(value: Value) -> ValueContract {
    ValueContract::result_from_value(&value, limits()).unwrap()
}

#[test]
fn nested_consumer_defaults_are_independent_and_never_replace_invalid_values() {
    let contract = inputs(json!({"people": {
        "type":"list", "required":true, "checks":[], "items":{
            "type":"object", "allow_extra_fields":false, "fields":{
                "name":{"type":"string", "required":true},
                "note":{"type":"string", "required":false, "nullable":true, "default":null},
                "tags":{"type":"list", "required":false, "default":[], "items":{"type":"string"}},
                "optional":{"type":"object", "required":false, "allow_extra_fields":false, "fields":{
                    "child":{"type":"integer", "required":false, "default":7}
                }}
            }
        }
    }}));
    let mut first = contract.bind(&json!({"people":[{"name":"Ada"}]})).unwrap();
    assert_eq!(
        first["people"],
        json!([{"name":"Ada","note":null,"tags":[]}])
    );
    first["people"][0]["tags"]
        .as_array_mut()
        .unwrap()
        .push(json!("private task A"));
    let second = contract
        .bind(&json!({"people":[{"name":"B","optional":{}}]}))
        .unwrap();
    assert_eq!(second["people"][0]["tags"], json!([]));
    assert_eq!(second["people"][0]["optional"], json!({"child":7}));
    for invalid in [
        json!({}),
        json!({"people":null}),
        json!({"people":[{}]}),
        json!({"people":[{"name":"Ada", "note":5}]}),
        json!({"people":[{"name":"Ada","extra":null}]}),
    ] {
        assert!(contract.bind(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn result_validation_preserves_absent_optional_fields_and_forbids_defaults_everywhere() {
    let shape = json!({"type":"object","allow_extra_fields":false,"fields":{
        "needed":{"type":"string","required":true},
        "optional":{"type":"string","required":false,"nullable":true}
    }});
    let contract = output(shape.clone());
    let value = json!({"needed":"actual completed effect"});
    contract.validate(&value).unwrap();
    assert_eq!(value, json!({"needed":"actual completed effect"}));
    assert!(contract.validate(&json!({})).is_err());
    let mut defaulted = shape;
    defaulted["fields"]["optional"]["default"] = Value::Null;
    assert!(ValueContract::result_from_value(&defaulted, limits()).is_err());
    for invalid in [
        json!({"type":"list","items":{"type":"string","default":"invented"}}),
        json!({"type":"object","fields":{},"allow_extra_fields":true,"extra_fields":{"type":"string","default":"invented"}}),
    ] {
        assert!(ValueContract::result_from_value(&invalid, limits()).is_err());
    }
}

#[test]
fn exact_numeric_constraints_unicode_and_cross_inputs_are_order_independent() {
    let high = 9_007_199_254_740_993u64;
    let contract = inputs(json!({
        "a":{"type":"integer","required":true,"checks":[{"kind":"max_field","input":"z"}]},
        "z":{"type":"integer","required":false,"default":high,"checks":[{"kind":"min","value":high},{"kind":"max","value":u64::MAX}]},
        "text":{"type":"string","required":true,"checks":[{"kind":"min_length","value":3}]}
    }));
    contract.bind(&json!({"a":high,"text":"üä🙂"})).unwrap();
    assert!(contract.bind(&json!({"a":high+1,"text":"üä🙂"})).is_err());
    assert!(
        contract
            .bind(&json!({"a":high,"z":high-1,"text":"üä🙂"}))
            .is_err()
    );
    assert!(contract.bind(&json!({"a":true,"text":"üä🙂"})).is_err());
    assert!(contract.bind(&json!({"a":1.0,"text":"üä🙂"})).is_err());
    assert!(contract.bind(&json!({"a":1,"text":"üä"})).is_err());
    let numeric = output(
        json!({"type":"number","checks":[{"kind":"min","value":-1e100},{"kind":"max","value":high}]}),
    );
    for valid in [
        json!(-1e100),
        json!(-0.0),
        json!(0),
        json!(high),
        json!(1e-200),
    ] {
        numeric.validate(&valid).unwrap();
    }
    assert!(numeric.validate(&json!(high + 1)).is_err());
    assert!(numeric.validate(&json!(-1e101)).is_err());
    assert!(numeric.validate(&json!("1")).is_err());
}

#[test]
fn schema_errors_and_duplicate_keys_fail_before_binding() {
    for invalid in [
        json!({"x":{"type":"null","required":true,"checks":[],"nullable":false}}),
        json!({"x":{"type":"string","required":false,"checks":[]}}),
        json!({"x":{"type":"string","required":true,"checks":[],"default":"bad"}}),
        json!({"x":{"type":"string","required":true}}),
        json!({"x":{"type":"integer","required":true,"checks":[{"kind":"min","value":3},{"kind":"max","value":2}]}}),
        json!({"x":{"type":"integer","required":true,"checks":[{"kind":"min","value":true}]}}),
        json!({"x":{"type":"list","required":true,"checks":[]}}),
        json!({"x":{"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":true}}),
        json!({"x":{"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":false,"extra_fields":{"type":"null"}}}),
        json!({"x":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1,"eval":"forbidden"}]}}),
        json!({"x":{"type":"integer","required":false,"checks":[],"default":"wrong"}}),
        json!({"x":{"type":"integer","required":true,"nullable":true,"checks":[{"kind":"max_field","input":"y"}]},"y":{"type":"integer","required":true,"checks":[]}}),
        json!({"x":{"type":"integer","required":true,"checks":[{"kind":"max_field","input":"undeclared"}]}}),
    ] {
        assert!(
            InputContract::from_value(&invalid, limits()).is_err(),
            "{invalid}"
        );
    }
    for duplicate in [
        r#"{"x":1,"x":2}"#,
        r#"{"nested":[{"type":"integer","type":"string"}]}"#,
        r#"{"key":1,"\u006bey":2}"#,
    ] {
        assert!(strict_json(duplicate, limits()).is_err());
    }
    assert!(strict_json("{} {}", limits()).is_err());
    assert!(strict_json("1e1000", limits()).is_err());
    assert!(strict_json("NaN", limits()).is_err());
}

#[test]
fn typed_extra_fields_and_nullable_parents_keep_their_declared_semantics() {
    let contract = inputs(json!({"mapping":{
        "type":"object","required":false,"default":null,"nullable":true,"checks":[],
        "fields":{},"allow_extra_fields":true,"extra_fields":{"type":"list","items":{"type":"integer"}}
    }}));
    assert_eq!(contract.bind(&json!({})).unwrap()["mapping"], Value::Null);
    assert_eq!(
        contract.bind(&json!({"mapping":null})).unwrap()["mapping"],
        Value::Null
    );
    contract.bind(&json!({"mapping":{"value":[1,2]}})).unwrap();
    assert!(contract.bind(&json!({"mapping":{"value":["2"]}})).is_err());
    assert!(contract.bind(&json!({"undeclared":null})).is_err());
}

#[test]
fn private_error_paths_retain_exact_diagnostics_without_leaking_dynamic_keys() {
    let contract = output(
        json!({"type":"object","fields":{},"allow_extra_fields":true,"extra_fields":{"type":"integer"}}),
    );
    let failure = contract
        .validate(&json!({"private/~user key":"wrong type"}))
        .unwrap_err();
    assert_eq!(failure.path(), "$/private~1~0user key");
    assert!(!format!("{failure:?} {failure}").contains("private"));
    assert!(!format!("{failure:?} {failure}").contains("wrong type"));
}

#[test]
fn computed_arguments_are_data_contracts_without_defaults_or_expression_execution() {
    let inputs = inputs(json!({"value":{"type":"integer","required":true,"checks":[]}}));
    let record = json!({"type":"integer","checks":[{"kind":"min","value":1}],"depends_on":["value"],"meaning":"Positive successor"});
    let contract = ValueContract::computed_from_json(&record.to_string(), &inputs).unwrap();
    contract.validate(&json!(2)).unwrap();
    assert!(contract.validate(&json!(0)).is_err());
    for bad in [json!(["missing"]), json!(["value", "value"]), json!([1])] {
        let mut value = record.clone();
        value["depends_on"] = bad;
        assert!(ValueContract::computed_from_json(&value.to_string(), &inputs).is_err());
    }
    for extra in ["default", "required", "eval"] {
        let mut value = record.clone();
        value[extra] = json!(true);
        assert!(ValueContract::computed_from_json(&value.to_string(), &inputs).is_err());
    }
}

#[test]
fn whole_value_compatibility_is_recursive_and_does_not_infer_presence_or_coercion() {
    let consumer = inputs(
        json!({"value":{"type":"object","required":true,"checks":[],"allow_extra_fields":true,
        "fields":{"number":{"type":"number","required":true,"checks":[{"kind":"min","value":0}]},"note":{"type":"string","required":false,"default":"missing"}},
        "extra_fields":{"type":"string"}}}),
    );
    let producer = json!({"type":"object","allow_extra_fields":false,"fields":{
        "number":{"type":"number","required":true,"checks":[{"kind":"min","value":1}]}
    }});
    output(producer.clone())
        .require_compatible_input(&consumer, "value")
        .unwrap();
    let mut different_type = producer.clone();
    different_type["fields"]["number"]["type"] = json!("integer");
    assert!(
        output(different_type)
            .require_compatible_input(&consumer, "value")
            .is_err()
    );
    let mut optional = producer.clone();
    optional["fields"]["number"]["required"] = json!(false);
    assert!(
        output(optional)
            .require_compatible_input(&consumer, "value")
            .is_err()
    );
    let mut nullable = producer.clone();
    nullable["fields"]["number"]["nullable"] = json!(true);
    assert!(
        output(nullable)
            .require_compatible_input(&consumer, "value")
            .is_err()
    );
    let mut insufficient = producer.clone();
    insufficient["fields"]["number"]["checks"] = json!([]);
    assert!(
        output(insufficient)
            .require_compatible_input(&consumer, "value")
            .is_err()
    );
    let mut extra = producer;
    extra["allow_extra_fields"] = json!(true);
    extra["extra_fields"] = json!({"type":"integer"});
    assert!(
        output(extra)
            .require_compatible_input(&consumer, "value")
            .is_err()
    );
    let closed = inputs(
        json!({"value":{"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":false}}),
    );
    let optional_extra = output(
        json!({"type":"object","fields":{"possible":{"type":"string","required":false}},"allow_extra_fields":false}),
    );
    assert!(
        optional_extra
            .require_compatible_input(&closed, "value")
            .is_err()
    );
}

#[test]
fn aggregate_limits_cover_default_expansion_and_reject_entire_values() {
    let limits = ContractLimits {
        max_depth: 32,
        max_nodes: 256,
        max_bytes: 4096,
    };
    let schema = json!({"items":{"type":"list","required":true,"checks":[],"items":{
        "type":"object","fields":{"tags":{"type":"list","required":false,
            "default":vec!["value";80],"items":{"type":"string"}}},"allow_extra_fields":false}}});
    let contract = InputContract::from_value(&schema, limits).unwrap();
    contract.bind(&json!({"items":[{},{}]})).unwrap();
    // One declared default can recur across many items. Reject during bounded
    // materialization, before constructing the oversized result.
    assert!(contract.bind(&json!({"items":[{},{},{},{}]})).is_err());
    let deep = json!([[[[null]]]]);
    assert!(
        strict_json(
            &deep.to_string(),
            ContractLimits {
                max_depth: 2,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        strict_json(
            "[]",
            ContractLimits {
                max_nodes: 0,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        strict_json(
            r#""private rejected payload""#,
            ContractLimits {
                max_bytes: 8,
                ..limits
            }
        )
        .is_err()
    );
}
