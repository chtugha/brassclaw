use super::*;
use crate::component_revision::RetainedComponentSnapshot;

pub(super) fn ids() -> ReviewComponents {
    ReviewComponents {
        recipe: Uuid::from_u128(1),
        prepare_request: Uuid::from_u128(2),
        usages: std::array::from_fn(|i| ReviewUsage {
            tool: Uuid::from_u128(100 + i as u128),
            tool_skill: Uuid::from_u128(200 + i as u128),
            skill: Uuid::from_u128(300 + i as u128),
            python_code: Uuid::from_u128(400 + i as u128),
            callable: format!("host.review_{}", OPERATIONS[i]),
            capability_id: format!("fixture.review_{}", OPERATIONS[i]),
        }),
    }
}

#[test]
fn coherent_graph_requires_real_tool_definitions_and_exact_skill_associations() {
    let ids = ids();
    let candidates = drafts(&ids).unwrap();
    assert!(
        RetainedComponentSnapshot::new(
            &[ids.recipe],
            candidates
                .iter()
                .cloned()
                .map(|d| d.at_version(1).unwrap())
                .collect()
        )
        .is_err()
    );
    let mut revisions = candidates
        .iter()
        .cloned()
        .map(|d| d.at_version(1).unwrap())
        .collect::<Vec<_>>();
    for usage in &ids.usages {
        revisions.push(draft(usage.tool,0,json!({"capability_id":usage.capability_id,
            "callable":usage.callable,"input_contract":{"record_bytes":{"type":"string","required":true,"checks":[]}},
            "result_contract":{"type":"string"}}),vec![],Value::Null).unwrap().at_version(1).unwrap());
    }
    let snapshot = RetainedComponentSnapshot::new(&[ids.recipe], revisions).unwrap();
    assert_eq!(verify_package(&ids, &snapshot).unwrap().len(), 30);
    for usage in &ids.usages {
        let association = snapshot.revisions()[&usage.skill]
            .draft()
            .association()
            .unwrap();
        assert_eq!(association.python_code_uuid(), usage.python_code);
        assert_eq!(association.tool_skill_uuid(), usage.tool_skill);
        assert_eq!(association.tool_uuid(), usage.tool);
    }
    let recipe = snapshot.revisions()[&ids.recipe].draft().document();
    // Private helpers cannot route ordinary user messages into Sempai learning.
    assert!(
        recipe["variants"][0]["intent_examples"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let steps = recipe["step_descriptions"][0]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 15);
    assert_eq!(steps[6]["include"], json!([ids.prepare_request]));
    for step in steps {
        assert_eq!(step["include"].as_array().unwrap().len(), 1);
    }
    // Deterministic compiler bytes; latest catalogue heads never enter drafts.
    assert_eq!(
        candidates
            .iter()
            .map(|d| d.exact_bytes())
            .collect::<Vec<_>>(),
        drafts(&ids)
            .unwrap()
            .iter()
            .map(|d| d.exact_bytes().to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn source_injection_and_identity_collisions_cannot_create_host_callables() {
    let mut ids = ids();
    ids.usages[0].callable = "host.read(record_bytes='forged'); host.write".into();
    assert!(drafts(&ids).is_err());
    let mut ids = self::ids();
    ids.usages[1].python_code = ids.usages[0].skill;
    assert!(drafts(&ids).is_err());
}
