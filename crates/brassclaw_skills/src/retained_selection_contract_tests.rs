//! Supported retained wire fixtures built from real immutable revision envelopes.
//! These are passive structure tests; approval records are deliberately not trusted.
use super::*;
use crate::component_revision::{ComponentRevisionDraft, REVISION_LIMITS};
use serde_json::json;

fn revision(
    id: u128,
    class: i32,
    document: Value,
    dependencies: &[Uuid],
) -> crate::component_revision::RetainedComponentRevision {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1", "uuid":Uuid::from_u128(id),
        "class_code":class,"document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )
    .unwrap()
    .at_version(1)
    .unwrap()
}
fn hex_ref(r: ComponentRevisionRef) -> Value {
    json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex_encode(&r.checksum)})
}
fn fixture() -> (Value, RetainedComponentSnapshot) {
    fixture_with_options(false, true)
}
fn fixture_with_result(result: bool) -> (Value, RetainedComponentSnapshot) {
    fixture_with_options(result, true)
}
fn fixture_with_options(result: bool, explicit: bool) -> (Value, RetainedComponentSnapshot) {
    fixture_with_export(result, explicit, "run_step")
}
fn fixture_with_export(
    result: bool,
    explicit: bool,
    public_name: &str,
) -> (Value, RetainedComponentSnapshot) {
    let code = |symbol: &str| {
        json!({"content":format!("def {symbol}(inputs):\n    return inputs[\"value\"]\n"),
        "input_contract":{"value":{"type":"string","required":true,"checks":[]}},"result_contract":{"type":"string"},
        "preload":{"format":"python-preload/2","exports":{symbol:{"symbol":symbol,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":symbol}})
    };
    let first = revision(1, 22, code(public_name), &[]);
    let second = revision(2, 22, code("finish"), &[]);
    let variant = json!({"variant_key":"selected","description":null,"step_link":"0:1-0:E", "intent_examples":[],"variable_patterns":[]});
    let layout = json!({"format":"recipe-input-layout/1", "task_inputs":{"value":{"type":"string","required":true,"checks":[]}},
        "steps":{"0:1":{"value":{"kind":"task_input","reference":"{{vars.value}}"}},
            "0:2":{"value":if result {json!({"kind":"result","step_id":"0:1","path":[]})} else {json!({"kind":"constant","value":"second"})}}}});
    let invocations = if explicit {
        json!({"selected":{"0:1":public_name,"0:2":"finish"}})
    } else {
        Value::Null
    };
    let mut document = json!({"variants":[variant],"input_layouts":{"selected":layout},"invocations":invocations,
        "step_descriptions":[{"desc_idx":0,"label":"fixture","yaml_source":"", "steps":[
            {"stepnumber":1,"knowledge":"orchestrator","type":"component","goal":"first","content":"","include":[first.reference().uuid]},
            {"stepnumber":2,"knowledge":"orchestrator","type":"component","goal":"second","content":"","include":[second.reference().uuid]}]}]});
    if !explicit {
        document.as_object_mut().unwrap().remove("invocations");
    }
    let recipe = revision(
        3,
        21,
        document,
        &[first.reference().uuid, second.reference().uuid],
    );
    let value = json!({"format":FORMAT_V1,"recipe":hex_ref(recipe.reference()),"variant":variant,
        "workflow_class":"deterministic","input_layout":layout,"invocation_layout":invocations,
        "step_order":["0:1","0:2"],"components":[hex_ref(first.reference()),hex_ref(second.reference()),hex_ref(recipe.reference())]});
    let snapshot =
        RetainedComponentSnapshot::new(&[recipe.reference().uuid], vec![first, second, recipe])
            .unwrap();
    (value, snapshot)
}
fn v3_fixture() -> (Value, RetainedComponentSnapshot) {
    v3_fixture_with_invocations(true)
}
fn v3_fixture_with_invocations(explicit: bool) -> (Value, RetainedComponentSnapshot) {
    let (mut value, snapshot) = fixture_with_options(false, explicit);
    let implementation = snapshot.revisions()[&Uuid::from_u128(1)].reference();
    value["format"] = json!(FORMAT_V3);
    value["functional_approvals"] = json!({"0:1":{
        "profile":"pure_own","implementation":native_ref_value(implementation), "export":"run_step", "interface_checksum":"a".repeat(64),
        "own_approvals":{implementation.uuid.to_string():{"root":native_ref_value(implementation),"approval_id":Uuid::from_u128(4),
            "approval_checksum":"b".repeat(64),"subject_checksum":"c".repeat(64)}}}});
    (value, snapshot)
}
fn parse(value: &Value) -> Result<SelectionVersion, SelectionContractError> {
    parse_selection_version(&value.to_string(), REVISION_LIMITS)
}
fn valid(value: &Value, snapshot: &RetainedComponentSnapshot) {
    parse(value).unwrap().require_snapshot(snapshot).unwrap();
}
fn rejected(
    mut value: Value,
    snapshot: &RetainedComponentSnapshot,
    mutate: impl FnOnce(&mut Value),
) {
    valid(&value, snapshot); // every negative starts from a valid complete fixture
    mutate(&mut value);
    assert!(matches!(
        parse(&value),
        Err(SelectionContractError::Invalid)
    ));
}

#[test]
fn v1_original_bytes_and_optional_field_presence_survive() {
    let (value, snapshot) = fixture();
    for optional in [
        None,
        Some(json!({"format":"recipe-model-input-selection/1","binding":{"selection":"default"}})),
    ] {
        let mut value = value.clone();
        if let Some(binding) = optional {
            value["model_input_binding"] = binding;
        }
        // Model-input identity is checked separately against its exact Recipe.
        let bytes = format!(" \n{}\n", value);
        let parsed = parse_selection_version(&bytes, REVISION_LIMITS).unwrap();
        assert!(matches!(&parsed, SelectionVersion::V1(_)));
        assert_eq!(parsed.selection().exact_bytes(), bytes);
        assert_eq!(parsed.selection().checksum(), sha256_of(bytes.as_bytes()));
        assert_eq!(
            parsed.selection().value().get("model_input_binding"),
            value.get("model_input_binding")
        );
    }
    valid(&value, &snapshot);
    let mut pins = value.clone();
    pins["association_approvals"] =
        json!({"0:2":{"approval_id":Uuid::from_u128(8),"checksum":"d".repeat(64)}});
    valid(&pins, &snapshot);
    for field in ["approved", "functional_approvals", "execution_profile"] {
        rejected(value.clone(), &snapshot, |v| v[field] = json!({}));
    }
}
#[test]
fn v1_and_v3_unknown_and_missing_fields_are_closed() {
    for (value, snapshot) in [fixture(), v3_fixture()] {
        for name in BASE_FIELDS {
            rejected(value.clone(), &snapshot, |v| {
                v.as_object_mut().unwrap().remove(*name);
            });
        }
        rejected(value.clone(), &snapshot, |v| {
            v["format"] = json!("monty-retained-recipe-selection/999")
        });
        rejected(value.clone(), &snapshot, |v| v["extra"] = json!(true));
        rejected(value.clone(), &snapshot, |v| {
            v["variant"]["extra"] = json!(true)
        });
        rejected(value.clone(), &snapshot, |v| {
            v["association_approvals"] = json!([])
        });
        rejected(value.clone(), &snapshot, |v| {
            v["model_input_binding"] = Value::Null
        });
    }
}
#[test]
fn base_references_keep_hex_codec_and_exact_unique_membership() {
    for (value, snapshot) in [fixture(), v3_fixture()] {
        rejected(value.clone(), &snapshot, |v| {
            v["recipe"]["checksum"] = json!(vec![0; 32])
        });
        rejected(value.clone(), &snapshot, |v| {
            v["recipe"]["uuid"] = json!(Uuid::nil())
        });
        rejected(value.clone(), &snapshot, |v| {
            v["recipe"]["version"] = json!(0)
        });
        rejected(value.clone(), &snapshot, |v| {
            v["components"][0]["version"] = json!(true)
        });
        rejected(value.clone(), &snapshot, |v| {
            v["components"][0]["checksum"] = json!("A".repeat(64))
        });
        rejected(value.clone(), &snapshot, |v| {
            let item = v["components"][0].clone();
            v["components"].as_array_mut().unwrap().push(item);
        });
        rejected(value.clone(), &snapshot, |v| {
            v["components"].as_array_mut().unwrap().pop();
        });
    }
}
#[test]
fn step_layout_and_input_reference_negatives_are_isolated() {
    for (value, snapshot) in [fixture(), v3_fixture()] {
        rejected(value.clone(), &snapshot, |v| {
            v["step_order"] = json!(["0:1", "0:1", "0:2"])
        });
        rejected(value.clone(), &snapshot, |v| {
            v["step_order"][0] = json!("step_1")
        });
        rejected(value.clone(), &snapshot, |v| {
            v["variant"]["step_link"] = json!("0:1-1:E")
        });
        rejected(value.clone(), &snapshot, |v| {
            v["input_layout"]["steps"]["0:3"] = json!({})
        });
        rejected(value.clone(), &snapshot, |v| {
            v["input_layout"]["steps"]["0:1"]["value"]["reference"] = json!("{{vars.missing}}")
        });
        rejected(value.clone(), &snapshot, |v| {
            v["input_layout"]["steps"]["0:1"]["value"]["reference"] = json!("prefix {{vars.value}}")
        });
        rejected(value.clone(), &snapshot, |v| {
            v["input_layout"]["steps"]["0:1"]["value"] =
                json!({"kind":"result","step_id":"0:2","path":[]})
        });
        rejected(value.clone(), &snapshot, |v| {
            v["input_layout"]["task_inputs"]["value"]["type"] = json!("untyped")
        });
        rejected(value.clone(), &snapshot, |v| {
            v["invocation_layout"]["selected"]["0:3"] = json!("finish")
        });
        rejected(value.clone(), &snapshot, |v| {
            v["invocation_layout"]["selected"]["0:2"] = json!("eval")
        });
    }
}
#[test]
fn v3_functional_pins_use_native_refs_and_match_selected_graph_and_export() {
    let (value, snapshot) = v3_fixture();
    let own = Uuid::from_u128(1).to_string();
    for (path, replacement) in [
        ("/functional_approvals/0:1/profile", json!("system")),
        ("/functional_approvals/0:1/implementation/version", json!(0)),
        (
            "/functional_approvals/0:1/implementation/uuid",
            json!(Uuid::nil()),
        ),
        (
            "/functional_approvals/0:1/implementation/class_code",
            json!(21),
        ),
        ("/functional_approvals/0:1/interface_checksum", json!("bad")),
        ("/functional_approvals/0:1/export", json!("finish")),
        ("/functional_approvals/0:1/own_approvals", json!({})),
    ] {
        rejected(value.clone(), &snapshot, |v| {
            *v.pointer_mut(path).unwrap() = replacement
        });
    }
    rejected(value.clone(), &snapshot, |v| {
        v["functional_approvals"]["0:1"]["own_approvals"][&own]["root"]["version"] = json!(0)
    });
    rejected(value.clone(), &snapshot, |v| {
        v["functional_approvals"]["0:1"]["own_approvals"][&own]["root"]["uuid"] = json!(Uuid::nil())
    });
    rejected(value.clone(), &snapshot, |v| {
        v["functional_approvals"]["0:1"]["own_approvals"][&own]["approval_id"] = json!(Uuid::nil())
    });
    rejected(
        value.clone(),
        &snapshot,
        |v| {
            v["association_approvals"] =
                json!({"0:1":{"approval_id":Uuid::from_u128(8),"checksum":"d".repeat(64)}})
        },
    );
    rejected(value.clone(), &snapshot, |v| {
        let pin = v["functional_approvals"]["0:1"].take();
        v["functional_approvals"]
            .as_object_mut()
            .unwrap()
            .remove("0:1");
        v["functional_approvals"]["0:3"] = pin;
    });
    rejected(value.clone(), &snapshot, |v| {
        v["functional_approvals"] = json!({})
    });
    let mut mixed = value.clone();
    mixed["association_approvals"] =
        json!({"0:2":{"approval_id":Uuid::from_u128(8),"checksum":"d".repeat(64)}});
    valid(&mixed, &snapshot);
    mixed["functional_approvals"]["0:1"]["profile"] = json!("authored_function");
    valid(&mixed, &snapshot);
}
#[test]
fn exact_recipe_variant_link_layout_and_step_implementation_are_checked() {
    let (value, snapshot) = v3_fixture();
    for mutation in 0..5 {
        let mut changed = value.clone();
        match mutation {
            0 => changed["variant"]["step_link"] = json!("0:2-0:E"),
            1 => changed["step_order"] = json!(["0:2", "0:1"]),
            2 => changed["variant"]["variant_key"] = json!("other"),
            3 => {
                changed["input_layout"]["steps"]["0:2"]["value"] =
                    json!({"kind":"constant","value":"other"})
            }
            4 => {
                let r = snapshot.revisions()[&Uuid::from_u128(2)].reference();
                changed["functional_approvals"]["0:1"]["implementation"] = native_ref_value(r);
                changed["functional_approvals"]["0:1"]["own_approvals"] = json!({r.uuid.to_string(): {
                    "root":native_ref_value(r),"approval_id":Uuid::from_u128(4),"approval_checksum":"b".repeat(64),"subject_checksum":"c".repeat(64)
                }});
            }
            _ => unreachable!(),
        }
        valid(&value, &snapshot);
        // Individual mutations can fail either the wire or exact-graph phase.
        assert!(
            parse(&changed)
                .and_then(|p| p.require_snapshot(&snapshot))
                .is_err()
        );
    }
}
#[test]
fn builder_roundtrip_is_deterministic_and_preserves_bytes_checksum_domain() {
    let (value, snapshot) = v3_fixture();
    let SelectionVersion::V3(parsed) = parse(&value).unwrap() else {
        panic!()
    };
    let build = || {
        build_v3_bytes(
            parsed.base(),
            parsed.association_approvals(),
            parsed.functional_approvals(),
            parsed.model_input_binding(),
            REVISION_LIMITS,
        )
        .unwrap()
    };
    let (a, hash) = build();
    let (b, _) = build();
    assert_eq!(a, b);
    assert_eq!(hash, sha256_of(a.as_bytes()));
    assert_eq!(a, value.to_string());
    let mut fresh = Value::Object(
        BASE_FIELDS
            .iter()
            .map(|key| ((*key).to_owned(), value[*key].clone()))
            .collect(),
    );
    let (fresh_bytes, fresh_checksum) = build_v3_bytes(
        BaseFields::from_value(&fresh).unwrap(),
        parsed.association_approvals(),
        parsed.functional_approvals(),
        parsed.model_input_binding(),
        REVISION_LIMITS,
    )
    .unwrap();
    assert_eq!(fresh_bytes, a);
    assert_eq!(fresh_checksum, hash);
    fresh["capture_preparation_bytes"] = json!("not a captured base field");
    assert!(BaseFields::from_value(&fresh).is_err());
    parse_selection_version(&a, REVISION_LIMITS)
        .unwrap()
        .require_snapshot(&snapshot)
        .unwrap();
}
#[test]
fn duplicate_keys_fail_in_an_otherwise_valid_document() {
    let (value, snapshot) = v3_fixture();
    valid(&value, &snapshot);
    let bytes = value.to_string();
    for duplicate in [
        bytes.replacen(
            "\"format\":",
            &format!("\"format\":\"{FORMAT_V3}\",\"format\":"),
            1,
        ),
        bytes.replacen(
            "\"profile\":\"pure_own\"",
            "\"profile\":\"pure_own\",\"profile\":\"pure_own\"",
            1,
        ),
    ] {
        assert_ne!(duplicate, bytes);
        assert!(matches!(
            parse_selection_version(&duplicate, REVISION_LIMITS),
            Err(SelectionContractError::Invalid)
        ));
    }
}
#[test]
fn captured_limits_apply_on_first_parse_and_builder_without_legacy_quota() {
    let (mut value, _) = v3_fixture();
    value["input_layout"]["steps"]["0:1"]["value"] =
        json!({"kind":"constant","value":"x".repeat(REVISION_LIMITS.max_bytes)});
    let bytes = value.to_string();
    let limits = ContractLimits {
        max_bytes: bytes.len(),
        ..REVISION_LIMITS
    };
    assert!(matches!(
        parse_selection_version(&bytes, REVISION_LIMITS),
        Err(SelectionContractError::Capacity)
    ));
    let SelectionVersion::V3(parsed) = parse_selection_version(&bytes, limits).unwrap() else {
        panic!()
    };
    assert!(
        build_v3_bytes(
            parsed.base(),
            parsed.association_approvals(),
            parsed.functional_approvals(),
            None,
            limits
        )
        .is_ok()
    );
    for reduced in [
        ContractLimits {
            max_bytes: bytes.len() - 1,
            ..limits
        },
        ContractLimits {
            max_nodes: 1,
            ..limits
        },
        ContractLimits {
            max_depth: 1,
            ..limits
        },
    ] {
        assert!(matches!(
            parse_selection_version(&bytes, reduced),
            Err(SelectionContractError::Capacity)
        ));
        assert!(matches!(
            build_v3_bytes(
                parsed.base(),
                parsed.association_approvals(),
                parsed.functional_approvals(),
                None,
                reduced
            ),
            Err(SelectionContractError::Capacity)
        ));
    }
}

#[test]
fn prior_result_binding_and_default_export_are_supported() {
    let (value, snapshot) = fixture_with_result(true);
    valid(&value, &snapshot);
    let (value, snapshot) = fixture_with_options(false, false);
    assert!(value["invocation_layout"].is_null());
    valid(&value, &snapshot);
}

#[test]
fn closed_pin_fields_and_unknown_export_are_rejected() {
    let (value, snapshot) = v3_fixture();
    for field in [
        "profile",
        "implementation",
        "export",
        "interface_checksum",
        "own_approvals",
    ] {
        rejected(value.clone(), &snapshot, |v| {
            v["functional_approvals"]["0:1"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        });
    }
    rejected(value.clone(), &snapshot, |v| {
        v["functional_approvals"]["0:1"]["unknown"] = json!(true)
    });
    rejected(value.clone(), &snapshot, |v| {
        v["functional_approvals"]["0:1"]["own_approvals"][Uuid::from_u128(1).to_string()]["unknown"] =
            json!(true)
    });
    let (mut value, snapshot) = fixture();
    value["invocation_layout"]["selected"]["0:1"] = json!("unknown_export");
    // Well-formed identifier is not proof of a declared selected export.
    // The exact Recipe/layout comparison also prevents changing its selection.
    assert!(parse(&value).unwrap().require_snapshot(&snapshot).is_err());
    for word in [
        "return", "class", "None", "yield", "lambda", "host", "inputs", "result", "eval", "exec",
        "open",
    ] {
        assert!(!is_valid_python_identifier(word));
    }
}

#[test]
fn reserved_q_v2_is_not_relabelled_but_shares_the_flat_base_boundary() {
    let (base, snapshot) = fixture();
    let mut q = base.clone();
    q["format"] = json!("monty-retained-recipe-selection/2");
    let capture = brassclaw_host_api::task_preparation::RetainedPreparation::encode(
        1,
        brassclaw_host_api::task_preparation::PreparationOrigin::AcknowledgedStartup,
        brassclaw_host_api::MontyExecutionLimits::default(),
    )
    .unwrap();
    q["capture_preparation_bytes"] = json!(capture.bytes());
    q["capture_preparation_checksum"] = json!(capture.checksum());
    q["execution_profile"] = json!("flat_stop/1");
    // Preserve Q's ARRAY of hex-wire own roots, not this package's native map.
    q["functional_approvals"] = json!({"0:1":{"profile":"pure_own","implementation":q["components"][0],
        "export":"run_step","interface_checksum":"a".repeat(64),"own_approvals":[{
            "root":q["components"][0],"approval_id":Uuid::from_u128(4),"approval_checksum":"b".repeat(64),"subject_checksum":"c".repeat(64)}]}});
    let exact = q.to_string();
    let borrowed = validate_base_wire(&q, REVISION_LIMITS).unwrap();
    borrowed.require_snapshot(&snapshot).unwrap();
    assert_eq!(
        borrowed.recipe(),
        parse(&base).unwrap().selection().recipe()
    );
    assert_eq!(q.to_string(), exact);
    assert!(matches!(parse(&q), Err(SelectionContractError::Invalid)));
    let mut relabel = q.clone();
    relabel["format"] = json!(FORMAT_V3);
    assert!(matches!(
        parse(&relabel),
        Err(SelectionContractError::Invalid)
    ));
    relabel["format"] = json!(FORMAT_V1);
    assert!(matches!(
        parse(&relabel),
        Err(SelectionContractError::Invalid)
    ));
    // The shared base boundary does not claim unsupported structured layouts.
    q["input_layout"]["format"] = json!("recipe-input-layout/2");
    assert!(validate_base_wire(&q, REVISION_LIMITS).is_err());
}

#[test]
fn authored_skill_only_roots_do_not_invent_an_own_code_approval() {
    let (value, _) = v3_fixture();
    let SelectionVersion::V3(parsed) = parse(&value).unwrap() else {
        panic!()
    };
    let mut pin = parsed.functional_approvals()["0:1"].clone();
    let mut own = pin.own_approvals.values().next().unwrap().clone();
    own.root.uuid = Uuid::from_u128(9);
    own.root.class_code = 1;
    pin.own_approvals = BTreeMap::from([(own.root.uuid.to_string(), own)]);
    pin.profile = ApprovalProfile::AuthoredFunction;
    // Passive root applicability only. B1 must still authenticate the actual
    // Skill/code partner and its unchanged prerequisite context.
    assert!(require_functional_roots(&pin).is_ok());
    pin.profile = ApprovalProfile::PureOwn;
    assert!(require_functional_roots(&pin).is_err());
    pin.profile = ApprovalProfile::AuthoredFunction;
    pin.own_approvals
        .values_mut()
        .next()
        .unwrap()
        .root
        .class_code = 22;
    assert!(require_functional_roots(&pin).is_err());
}

#[test]
fn public_export_selection_rejects_private_names_at_each_selection_boundary() {
    for private in ["_private", "_helper"] {
        // General/private symbols retain the original Engine grammar.
        assert!(is_valid_python_identifier(private));
        let (value, snapshot) = v3_fixture_with_invocations(false);
        rejected(value, &snapshot, |v| {
            v["functional_approvals"]["0:1"]["export"] = json!(private)
        });
        let (value, snapshot) = fixture();
        rejected(value, &snapshot, |v| {
            v["invocation_layout"]["selected"]["0:1"] = json!(private)
        });
        // No explicit invocation: exact graph checks must reject a private
        // default export even when its declaration/name/identity agree.
        let (positive, positive_graph) = fixture_with_export(false, false, "Case2");
        valid(&positive, &positive_graph);
        let (negative, negative_graph) = fixture_with_export(false, false, private);
        let parsed = parse(&negative).unwrap();
        assert!(matches!(
            parsed.require_snapshot(&negative_graph),
            Err(SelectionContractError::Invalid)
        ));
    }
    for public in ["run_step", "Case2"] {
        let (value, snapshot) = fixture_with_export(false, true, public);
        valid(&value, &snapshot);
    }
}
