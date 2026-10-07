//! Structured declarations/compatibility only. These fixtures are not approved
//! components, successful behavioral evidence or Tool invocation grants.
use brassclaw_skills::{
    association_contract::{
        AssociationApprovalDeclaration, ComponentRevisionRef, FailureAction, Idempotency,
        SkillAssociation, ValidationMode,
    },
    value_contract::{ContractLimits, InputContract},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn limits() -> ContractLimits {
    ContractLimits {
        max_depth: 32,
        max_nodes: 4096,
        max_bytes: 65536,
    }
}
fn association() -> Value {
    json!({
        "format":"skill-association/1",
        "skill_uuid":"11111111-1111-4111-8111-111111111111",
        "python_code_uuid":"22222222-2222-4222-8222-222222222222",
        "tool_skill_uuid":"33333333-3333-4333-8333-333333333333",
        "tool_uuid":"44444444-4444-4444-8444-444444444444",
        "callable":"host.read_file",
        "inputs":{
            "path":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},
            "start_line":{"type":"integer","required":true,"checks":[{"kind":"min","value":1},{"kind":"max","value":2147483647},{"kind":"max_field","input":"end_line"}]},
            "end_line":{"type":"integer","required":true,"checks":[{"kind":"min","value":1},{"kind":"max","value":2147483647}]}
        },
        "arguments":{"path":"path","offset":"start_line"},
        "code_arguments":{"limit":{"type":"integer","depends_on":["start_line","end_line"],"checks":[{"kind":"min","value":1},{"kind":"max","value":2147483647}],"meaning":"Inclusive interval length"}},
        "result":{"type":"object","fields":{"content":{"type":"string","required":true}},"allow_extra_fields":false},
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}
    })
}
fn parse(value: &Value) -> SkillAssociation {
    SkillAssociation::from_json(&value.to_string(), limits()).unwrap()
}
fn selected(a: &SkillAssociation) -> Vec<ComponentRevisionRef> {
    [
        (a.skill_uuid(), 1),
        (a.python_code_uuid(), 22),
        (a.tool_skill_uuid(), 13),
        (a.tool_uuid(), 0),
        (Uuid::from_u128(55), 22),
    ]
    .into_iter()
    .map(|(uuid, class_code)| ComponentRevisionRef {
        uuid,
        class_code,
        version: 1,
        checksum: [5; 32],
    })
    .collect()
}
fn approval(a: &SkillAssociation) -> Value {
    json!({"format":"skill-association-approval/1","approval_id":Uuid::from_u128(99),
        "association_checksum":format!("{:x}",Sha256::digest(a.exact_bytes().as_bytes())),
        "components": selected(a).into_iter().map(|c| json!({"uuid":c.uuid,"class_code":c.class_code,"version":c.version,"checksum":"05".repeat(32)})).collect::<Vec<_>>(),
        "validation_mode":"authored","q1_ref":"evidence:q1","q2_ref":"evidence:human-q2","behavioral_refs":["evidence:observed-behavior"]})
}

#[test]
fn interval_usage_checks_actual_arguments_and_registered_required_parameter_contract() {
    let a = parse(&association());
    let tool = InputContract::from_json(&json!({
        "path":{"type":"string","required":true,"checks":[]},
        "offset":{"type":"integer","required":true,"checks":[{"kind":"min","value":0},{"kind":"max","value":u64::MAX}]},
        "limit":{"type":"integer","required":true,"checks":[{"kind":"min","value":0},{"kind":"max","value":u64::MAX}]}
    }).to_string(),limits()).unwrap();
    a.require_tool_input_contract(&tool).unwrap();
    let hostile = "\"\nresult = host.shell(command='injected')\n{{vars.path}}";
    let supplied = json!({"path":hostile,"start_line":2,"end_line":3});
    a.validate_arguments(&supplied, &json!({"path":hostile,"offset":2,"limit":2}))
        .unwrap();
    let checksum = a.checksum();
    for bad in [
        json!({"path":"different","offset":2,"limit":2}),
        json!({"path":hostile,"offset":2.0,"limit":2}),
        json!({"path":hostile,"offset":2,"limit":0}),
        json!({"path":hostile,"offset":2,"limit":2147483648_u64}),
        json!({"path":hostile,"offset":2,"limit":true}),
        json!({"path":hostile,"offset":2,"limit":2,"grant":true}),
        json!({"path":hostile,"unexpected":2,"limit":2}),
    ] {
        assert!(a.validate_arguments(&supplied, &bad).is_err(), "{bad}");
    }
    assert!(
        a.validate_arguments(
            &json!({"path":hostile,"start_line":4,"end_line":3}),
            &json!({"path":hostile,"offset":4,"limit":1})
        )
        .is_err()
    );
    let incompatible = InputContract::from_json(
        &json!({"missing_required":{"type":"string","required":true,"checks":[]}}).to_string(),
        limits(),
    )
    .unwrap();
    assert!(a.require_tool_input_contract(&incompatible).is_err());
    a.result()
        .validate(&json!({"content":"successful empty data"}))
        .unwrap();
    assert!(a.result().validate(&json!({"content":null})).is_err());
    assert_eq!(a.checksum(), checksum);
    assert_eq!(a.failure().action(), FailureAction::Stop);
    assert_eq!(a.failure().max_attempts(), 1);
}

#[test]
fn exact_usage_format_rejects_unknown_duplicate_and_unresolved_arguments() {
    for (field, value) in [
        ("unexpected", json!(true)),
        ("skill_uuid", json!(Uuid::nil())),
        ("callable", json!("host.read_file(path='code')")),
        ("arguments", json!({"path":"undeclared"})),
        ("arguments", json!({"limit":"end_line"})),
        (
            "code_arguments",
            json!({"limit":{"type":"integer","checks":[],"depends_on":["undeclared"],"meaning":"unresolved"}}),
        ),
    ] {
        let mut v = association();
        v[field] = value;
        assert!(SkillAssociation::from_json(&v.to_string(), limits()).is_err());
    }
    let mut v = association();
    v["python_code_uuid"] = v["skill_uuid"].clone();
    assert!(SkillAssociation::from_json(&v.to_string(), limits()).is_err());
    let bytes = association().to_string();
    for bad in [
        bytes.replacen(
            "\"format\":",
            "\"format\":\"skill-association/1\",\"format\":",
            1,
        ),
        bytes.replacen(
            "\"path\":\"path\"",
            "\"path\":\"path\",\"path\":\"start_line\"",
            1,
        ),
    ] {
        assert!(SkillAssociation::from_json(&bad, limits()).is_err());
    }
    let mut v = association();
    v["result"]["fields"]["content"]["default"] = json!("fabricated");
    assert!(SkillAssociation::from_json(&v.to_string(), limits()).is_err());
}

#[test]
fn retries_require_exact_declared_outcomes_and_positive_total_attempts() {
    let mut v = association();
    v["failure"] = json!({"action":"retry","max_attempts":3,"idempotency":"deduplicated","idempotency_evidence_ref":"evidence:durable-dedup-contract","retryable_outcomes":["unknown_completion"]});
    let a = parse(&v);
    assert_eq!(a.failure().max_attempts(), 3);
    assert_eq!(a.failure().idempotency(), Idempotency::Deduplicated);
    assert_eq!(
        a.failure().evidence_reference(),
        Some("evidence:durable-dedup-contract")
    );
    for (field, value) in [
        ("max_attempts", json!(0)),
        ("max_attempts", json!(1)),
        ("max_attempts", json!(true)),
        ("max_attempts", json!(2.5)),
        ("idempotency", json!("not_assumed")),
        ("idempotency_evidence_ref", Value::Null),
        ("retryable_outcomes", json!([])),
        ("retryable_outcomes", json!(["timeout"])),
        (
            "retryable_outcomes",
            json!(["unknown_completion", "unknown_completion"]),
        ),
        ("extra", json!("not accepted")),
    ] {
        let mut bad = v.clone();
        bad["failure"][field] = value;
        assert!(SkillAssociation::from_json(&bad.to_string(), limits()).is_err());
    }
    let mut bad = association();
    bad["failure"]["max_attempts"] = json!(2);
    assert!(SkillAssociation::from_json(&bad.to_string(), limits()).is_err());
}

#[test]
fn approval_selection_compares_exact_bytes_roles_and_the_entire_supplied_graph() {
    let source = association().to_string();
    let a = SkillAssociation::from_json(&source, limits()).unwrap();
    let doc = approval(&a);
    let declaration =
        AssociationApprovalDeclaration::from_json(&doc.to_string(), limits()).unwrap();
    assert_eq!(declaration.mode(), ValidationMode::Authored);
    assert_eq!(declaration.q2_reference(), Some("evidence:human-q2"));
    assert_eq!(
        declaration.behavioral_references(),
        &["evidence:observed-behavior".to_owned()]
    );
    let selected = selected(&a);
    declaration
        .require_selected_combination(&a, &selected)
        .unwrap();
    assert!(
        declaration
            .require_selected_combination(&a, &selected[..4])
            .is_err()
    );
    for field in 0..3 {
        let mut newer = selected.clone();
        match field {
            0 => newer[4].version = 2,
            1 => newer[4].checksum = [6; 32],
            _ => newer[4].class_code = 13,
        }
        assert!(
            declaration
                .require_selected_combination(&a, &newer)
                .is_err()
        );
    }
    let reformatted = SkillAssociation::from_json(&format!(" {source}"), limits()).unwrap();
    assert!(
        declaration
            .require_selected_combination(&reformatted, &selected)
            .is_err()
    );
    let mut duplicated = selected.clone();
    duplicated.push(selected[0]);
    assert!(
        declaration
            .require_selected_combination(&a, &duplicated)
            .is_err()
    );
    let mut wrong_class = doc.clone();
    wrong_class["components"][0]["class_code"] = json!(10);
    let wrong =
        AssociationApprovalDeclaration::from_json(&wrong_class.to_string(), limits()).unwrap();
    let mut selected_wrong = selected.clone();
    selected_wrong[0].class_code = 10;
    assert!(
        wrong
            .require_selected_combination(&a, &selected_wrong)
            .is_err()
    );
}

#[test]
fn evidence_declarations_are_strict_and_do_not_promote_themselves_to_approvals() {
    let a = parse(&association());
    let doc = approval(&a);
    for (field, value) in [
        ("approval_id", json!(Uuid::nil())),
        ("association_checksum", json!("AA".repeat(32))),
        ("q1_ref", json!(" ")),
        ("q2_ref", Value::Null),
        ("behavioral_refs", json!([])),
        ("approved", json!(true)),
        ("validation_mode", json!("automatic-human-approval")),
    ] {
        let mut bad = doc.clone();
        bad[field] = value;
        assert!(AssociationApprovalDeclaration::from_json(&bad.to_string(), limits()).is_err());
    }
    for (field, value) in [
        ("version", json!(0)),
        ("version", json!(true)),
        ("version", json!(1.0)),
        ("class_code", json!(true)),
        ("checksum", json!("0")),
        ("grant", json!(true)),
    ] {
        let mut bad = doc.clone();
        bad["components"][0][field] = value;
        assert!(AssociationApprovalDeclaration::from_json(&bad.to_string(), limits()).is_err());
    }
    let mut duplicate = doc.clone();
    let component = duplicate["components"][0].clone();
    duplicate["components"]
        .as_array_mut()
        .unwrap()
        .push(component);
    assert!(AssociationApprovalDeclaration::from_json(&duplicate.to_string(), limits()).is_err());
    let mut seed = doc.clone();
    seed["validation_mode"] = json!("system_seed");
    assert!(AssociationApprovalDeclaration::from_json(&seed.to_string(), limits()).is_err());
    seed["q2_ref"] = Value::Null;
    // Structurally valid seed declarations still need the trusted bootstrap
    // store/evidence checks; this parser exposes no activation or permission API.
    let seed = AssociationApprovalDeclaration::from_json(&seed.to_string(), limits()).unwrap();
    assert_eq!(seed.mode(), ValidationMode::SystemSeed);
    assert_eq!(seed.q2_reference(), None);
    let bytes = doc.to_string();
    let duplicate = bytes.replacen("\"version\":1", "\"version\":1,\"version\":2", 1);
    assert!(AssociationApprovalDeclaration::from_json(&duplicate, limits()).is_err());
    let tiny = ContractLimits {
        max_bytes: bytes.len() - 1,
        ..limits()
    };
    assert!(AssociationApprovalDeclaration::from_json(&bytes, tiny).is_err());
}
