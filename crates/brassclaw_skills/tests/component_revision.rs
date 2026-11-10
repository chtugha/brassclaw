//! Neutral immutable graph checks. These fixtures confer no component approval
//! and do not claim execution or catalogue activation.
use brassclaw_skills::component_revision::{
    ComponentRevisionDraft, RetainedComponentRevision, RetainedComponentSnapshot,
};
use serde_json::json;
use uuid::Uuid;

fn revision(id: Uuid, dependencies: &[Uuid]) -> RetainedComponentRevision {
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1", "uuid":id, "class_code":22,
            "document":{"label":"graph fixture"},
            "dependencies":dependencies, "association":null,
        })
        .to_string(),
    )
    .unwrap()
    .at_version(1)
    .unwrap()
}

#[test]
fn deep_exact_graph_has_no_recursive_depth_quota_and_keeps_integrity_checks() {
    let ids: Vec<_> = (1..=2048).map(Uuid::from_u128).collect();
    let chain = || {
        ids.iter()
            .enumerate()
            .map(|(index, id)| {
                revision(
                    *id,
                    ids.get(index + 1).map(std::slice::from_ref).unwrap_or(&[]),
                )
            })
            .collect::<Vec<_>>()
    };
    let exact = chain();
    let references: Vec<_> = exact.iter().map(|revision| revision.reference()).collect();
    let selected = RetainedComponentSnapshot::new(&[ids[0]], exact).unwrap();
    assert_eq!(selected.revisions().len(), ids.len());
    for reference in references {
        assert_eq!(selected.revisions()[&reference.uuid].reference(), reference);
    }
    let mut cycle = chain();
    *cycle.last_mut().unwrap() = revision(*ids.last().unwrap(), &[ids[0]]);
    assert!(RetainedComponentSnapshot::new(&[ids[0]], cycle).is_err());
    let mut missing = chain();
    missing.pop();
    assert!(RetainedComponentSnapshot::new(&[ids[0]], missing).is_err());
    let mut unrelated = chain();
    unrelated.push(revision(Uuid::from_u128(9999), &[]));
    assert!(RetainedComponentSnapshot::new(&[ids[0]], unrelated).is_err());
    let mut duplicate = chain();
    duplicate.push(revision(ids[0], &[]));
    assert!(RetainedComponentSnapshot::new(&[ids[0]], duplicate).is_err());
}

#[test]
fn multiple_roots_and_shared_dependencies_do_not_look_like_cycles() {
    let [a, b, c, d] = [1, 2, 3, 4].map(Uuid::from_u128);
    let graph = vec![
        revision(a, &[b, c]),
        revision(b, &[d]),
        revision(c, &[d]),
        revision(d, &[]),
    ];
    let selected = RetainedComponentSnapshot::new(&[a, c], graph).unwrap();
    assert_eq!(selected.revisions().len(), 4);
    assert_eq!(selected.roots().len(), 2);
}

#[test]
fn selecting_roots_preserves_exact_versions_and_never_rereads_or_keeps_unrelated_roots() {
    let [a, b, c, d] = [21, 22, 23, 24].map(Uuid::from_u128);
    let full = RetainedComponentSnapshot::new(
        &[a, b],
        vec![
            revision(a, &[c]),
            revision(b, &[d]),
            revision(c, &[d]),
            revision(d, &[]),
        ],
    )
    .unwrap();
    let selected = full.select_roots(&[a]).unwrap();
    assert_eq!(
        selected.roots().iter().copied().collect::<Vec<_>>(),
        vec![a]
    );
    assert_eq!(selected.revisions().len(), 3);
    assert!(!selected.revisions().contains_key(&b));
    for (id, value) in selected.revisions() {
        assert_eq!(value.reference(), full.revisions()[id].reference());
        assert_eq!(
            value.draft().exact_bytes(),
            full.revisions()[id].draft().exact_bytes()
        );
    }
    assert!(
        selected.select_roots(&[b]).is_err(),
        "a source outside this retained view cannot be fetched later"
    );
    assert!(full.select_roots(&[a, a]).is_err());
    assert!(full.select_roots(&[]).is_err());
    assert!(full.select_roots(&[Uuid::nil()]).is_err());
    assert_eq!(
        full.revisions().len(),
        4,
        "selection leaves the original graph intact"
    );
}

// Codec fixtures below establish parser provenance only, never runtime ACK or
// approval. Actual acknowledged HTTP/PG coverage lives in snapshot_policy_tests.
fn parser_preparation(
    bytes: u64,
    nodes: u64,
    depth: u32,
) -> brassclaw_host_api::task_preparation::RetainedPreparation {
    use brassclaw_host_api::{
        MontyExecutionLimits,
        task_preparation::{PreparationOrigin, RetainedPreparation},
    };
    let policy = MontyExecutionLimits {
        max_value_bytes: bytes,
        max_value_nodes: nodes,
        max_value_depth: depth,
        ..MontyExecutionLimits::default()
    };
    RetainedPreparation::encode(7, PreparationOrigin::AcknowledgedLive, policy).unwrap()
}

#[test]
fn original_revision_parser_bytes_nodes_and_depth_have_exact_separate_bounds() {
    use brassclaw_skills::component_revision::REVISION_LIMITS;
    let id = Uuid::new_v4();
    let bytes = json!({"format":"component-revision/1","uuid":id,"class_code":23,
        "document":{"text":"x".repeat(REVISION_LIMITS.max_bytes)},"dependencies":[],"association":null}).to_string();
    assert!(ComponentRevisionDraft::from_json(&bytes).is_err());
    let original = parser_preparation(bytes.len() as u64, 1_000_000, 48);
    let parsed =
        ComponentRevisionDraft::from_json_with_preparation(&bytes, Some(&original)).unwrap();
    assert_eq!(parsed.exact_bytes(), bytes);
    assert_eq!(parsed.preparation().unwrap().bytes(), original.bytes());
    let smaller = parser_preparation(bytes.len() as u64 - 1, 1_000_000, 48);
    assert!(ComponentRevisionDraft::from_json_with_preparation(&bytes, Some(&smaller)).is_err());
    assert_eq!(
        parsed.exact_bytes(),
        bytes,
        "new policy cannot mutate a parsed original"
    );
    let bytes = json!({"format":"component-revision/1","uuid":id,"class_code":23,
        "document":{"items":vec![0; 300_000]},"dependencies":[],"association":null})
    .to_string();
    assert!(ComponentRevisionDraft::from_json(&bytes).is_err());
    let original = parser_preparation(8_388_608, 1_000_000, 48);
    assert!(ComponentRevisionDraft::from_json_with_preparation(&bytes, Some(&original)).is_ok());
    // Count the complete small envelope: root1 + six key/value pairs12 +
    // the document's key/value pair2. Object names are real counted nodes.
    let small = json!({"format":"component-revision/1","uuid":id,"class_code":23,
        "document":{"text":"x"},"dependencies":[],"association":null})
    .to_string();
    let at_nodes = parser_preparation(8_388_608, 15, 48);
    let below_nodes = parser_preparation(8_388_608, 14, 48);
    assert!(ComponentRevisionDraft::from_json_with_preparation(&small, Some(&at_nodes)).is_ok());
    assert!(
        ComponentRevisionDraft::from_json_with_preparation(&small, Some(&below_nodes)).is_err()
    );
    let at_depth = parser_preparation(8_388_608, 100, 2);
    let below_depth = parser_preparation(8_388_608, 100, 1);
    assert!(ComponentRevisionDraft::from_json_with_preparation(&small, Some(&at_depth)).is_ok());
    assert!(
        ComponentRevisionDraft::from_json_with_preparation(&small, Some(&below_depth)).is_err()
    );
    let depth = parser_preparation(8_388_608, 1_000_000, 65);
    assert!(matches!(
        ComponentRevisionDraft::from_json_with_preparation("{}", Some(&depth)),
        Err(brassclaw_skills::component_revision::RevisionError::UnsupportedDepth)
    ));
}

#[test]
fn association_schema_uses_its_revision_original_nodes_without_a_legacy_reparse() {
    let [id, code, binding, tool] = [41, 42, 43, 44].map(Uuid::from_u128);
    let fields: serde_json::Map<String, serde_json::Value> = (0..90_000)
        .map(|n| {
            (
                format!("field_{n}"),
                json!({"type":"string","required":false}),
            )
        })
        .collect();
    // Each optional field is a key plus an object with two key/value pairs.
    assert!(fields.len() * 6 > brassclaw_skills::component_revision::REVISION_LIMITS.max_nodes);
    let association = json!({"format":"skill-association/1","skill_uuid":id,"python_code_uuid":code,
        "tool_skill_uuid":binding,"tool_uuid":tool,"callable":"host.read_file","inputs":{},"arguments":{},"code_arguments":{},
        "result":{"type":"object","allow_extra_fields":false,"fields":fields},
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}}).to_string();
    let bytes = json!({"format":"component-revision/1","uuid":id,"class_code":1,
        "document":{"content":"One usage"},"dependencies":[code,binding,tool],"association":association}).to_string();
    assert!(bytes.len() < brassclaw_skills::component_revision::REVISION_LIMITS.max_bytes);
    assert!(ComponentRevisionDraft::from_json(&bytes).is_err());
    let original = parser_preparation(32 * 1024 * 1024, 1_000_000, 48);
    let parsed =
        ComponentRevisionDraft::from_json_with_preparation(&bytes, Some(&original)).unwrap();
    assert!(parsed.association().is_some());
    assert_eq!(parsed.parsing_limits().max_nodes, 1_000_000);
}
