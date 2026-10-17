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
