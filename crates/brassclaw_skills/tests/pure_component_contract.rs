//! Declaration/selection checks only. These fixtures supply no trusted review
//! records, system-seed provenance, component activation or execution grant.
use brassclaw_skills::{
    association_contract::{ComponentRevisionRef, ValidationMode},
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
    pure_component_contract::PureComponentApprovalDeclaration,
    value_contract::ContractLimits,
};
use serde_json::{Value, json};
use uuid::Uuid;

fn limits() -> ContractLimits {
    ContractLimits {
        max_depth: 32,
        max_nodes: 4096,
        max_bytes: 65_536,
    }
}
fn reference(reference: ComponentRevisionRef) -> Value {
    json!({"uuid":reference.uuid,"class_code":reference.class_code,
        "version":reference.version,"checksum":hex(reference.checksum)})
}
fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn snapshot(
    root_version: u64,
    helper_version: u64,
    helper_label: &str,
) -> RetainedComponentSnapshot {
    let root = Uuid::from_u128(1);
    let helper = Uuid::from_u128(2);
    let draft = |id, dependencies, label| {
        ComponentRevisionDraft::from_json(
            &json!({
                "format":"component-revision/1","uuid":id,"class_code":22,
                "document":{"content":"def identity(value):\n    return value\n","label":label},
                "dependencies":dependencies,"association":null
            })
            .to_string(),
        )
        .unwrap()
    };
    RetainedComponentSnapshot::new(
        &[root],
        vec![
            draft(root, vec![helper], "root")
                .at_version(root_version)
                .unwrap(),
            draft(helper, vec![], helper_label)
                .at_version(helper_version)
                .unwrap(),
        ],
    )
    .unwrap()
}
fn declaration(snapshot: &RetainedComponentSnapshot) -> Value {
    let root = snapshot.revisions()[&Uuid::from_u128(1)].reference();
    json!({"format":"pure-component-approval/1","approval_id":Uuid::from_u128(10),
        "root":reference(root),"components":snapshot.revisions().values().map(|r|reference(r.reference())).collect::<Vec<_>>(),
        "validation_mode":"authored","q1_ref":Uuid::from_u128(11),"q2_ref":Uuid::from_u128(12),
        "behavioral_refs":[Uuid::from_u128(13),Uuid::from_u128(14)]})
}

#[test]
fn pure_declaration_retains_exact_bytes_and_distinct_review_references() {
    let snapshot = snapshot(1, 1, "original helper");
    let bytes = format!("  {}\n", declaration(&snapshot));
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();
    assert_eq!(declaration.exact_bytes(), bytes);
    assert_eq!(declaration.approval_id(), Uuid::from_u128(10));
    assert_eq!(
        declaration.root(),
        snapshot.revisions()[&Uuid::from_u128(1)].reference()
    );
    assert_eq!(declaration.components().len(), 2);
    assert_eq!(declaration.mode(), ValidationMode::Authored);
    assert_eq!(declaration.q1_reference(), Uuid::from_u128(11));
    assert_eq!(declaration.q2_reference(), Some(Uuid::from_u128(12)));
    assert_eq!(
        declaration.behavioral_references(),
        &[Uuid::from_u128(13), Uuid::from_u128(14)]
    );
    declaration.require_selected_snapshot(&snapshot).unwrap();
}

#[test]
fn pure_declaration_compares_the_complete_exact_rooted_closure() {
    let original = snapshot(1, 1, "original helper");
    let declaration =
        PureComponentApprovalDeclaration::from_json(&declaration(&original).to_string(), limits())
            .unwrap();
    for replacement in [
        snapshot(2, 1, "original helper"),
        snapshot(1, 2, "original helper"),
        snapshot(1, 1, "changed helper bytes"),
        original.select_roots(&[Uuid::from_u128(2)]).unwrap(),
    ] {
        assert!(declaration.require_selected_snapshot(&replacement).is_err());
    }
    // Selecting a successor does not invalidate the retained original.
    declaration.require_selected_snapshot(&original).unwrap();
}

#[test]
fn pure_declaration_separates_authored_and_shipped_seed_evidence_shapes() {
    let original = declaration(&snapshot(1, 1, "original helper"));
    let mut seed = original.clone();
    seed["validation_mode"] = json!("system_seed");
    assert!(PureComponentApprovalDeclaration::from_json(&seed.to_string(), limits()).is_err());
    seed["q2_ref"] = Value::Null;
    let declaration =
        PureComponentApprovalDeclaration::from_json(&seed.to_string(), limits()).unwrap();
    assert_eq!(declaration.mode(), ValidationMode::SystemSeed);
    assert_eq!(declaration.q2_reference(), None);
    let mut authored = original;
    authored["q2_ref"] = Value::Null;
    assert!(PureComponentApprovalDeclaration::from_json(&authored.to_string(), limits()).is_err());
    // Neither a parsed mode nor a null Q2 reference proves trusted provenance.
}

#[test]
fn pure_declaration_rejects_incomplete_conflicting_or_reused_records() {
    let base = declaration(&snapshot(1, 1, "original helper"));
    let mut invalid = Vec::new();
    for (key, value) in [
        ("format", json!("skill-association-approval/1")),
        ("approval_id", json!(Uuid::nil())),
        ("q1_ref", json!(Uuid::nil())),
        ("q2_ref", base["q1_ref"].clone()),
        ("behavioral_refs", json!([])),
        ("behavioral_refs", json!([base["q1_ref"].clone()])),
        ("behavioral_refs", json!([base["q2_ref"].clone()])),
        (
            "behavioral_refs",
            json!([Uuid::from_u128(13), Uuid::from_u128(13)]),
        ),
        ("components", json!([])),
        ("components", json!([base["components"][1].clone()])),
        ("validation_mode", json!("approved")),
    ] {
        let mut changed = base.clone();
        changed[key] = value;
        invalid.push(changed);
    }
    for value in [
        json!(true),
        json!(0),
        json!((i64::MAX as u64) + 1),
        json!(1.5),
    ] {
        let mut changed = base.clone();
        changed["root"]["version"] = value.clone();
        changed["components"][0]["version"] = value;
        invalid.push(changed);
    }
    for (field, value) in [
        ("class_code", json!(0)),
        ("checksum", json!("A".repeat(64))),
        ("checksum", json!("a".repeat(63))),
        ("uuid", json!(Uuid::nil())),
        ("extra", json!("undeclared")),
    ] {
        let mut changed = base.clone();
        changed["components"][1][field] = value;
        invalid.push(changed);
    }
    let mut duplicate = base.clone();
    duplicate["components"]
        .as_array_mut()
        .unwrap()
        .push(base["components"][0].clone());
    invalid.push(duplicate);
    let mut extra = base.clone();
    extra["approved"] = json!(true);
    invalid.push(extra);
    for value in invalid {
        assert!(
            PureComponentApprovalDeclaration::from_json(&value.to_string(), limits()).is_err(),
            "invalid declaration accepted: {value}"
        );
    }
}

#[test]
fn pure_declaration_rejects_duplicate_json_keys_and_obeys_caller_bounds() {
    let bytes = declaration(&snapshot(1, 1, "original helper")).to_string();
    let duplicate = bytes.replacen("{", "{\"format\":\"pure-component-approval/1\",", 1);
    assert!(PureComponentApprovalDeclaration::from_json(&duplicate, limits()).is_err());
    let mut small = limits();
    small.max_bytes = 32;
    assert!(PureComponentApprovalDeclaration::from_json(&bytes, small).is_err());
}
