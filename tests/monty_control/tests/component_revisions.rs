//! Actual immutable PostgreSQL retention. These drafts are not approved
//! components, activation evidence or a replacement for production IBS tests.
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

fn draft(id: Uuid, class: i32, document: Value, dependencies: &[Uuid]) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1", "uuid":id, "class_code":class,
            "document":document, "dependencies":dependencies, "association":null,
        })
        .to_string(),
    )
    .unwrap()
}

#[tokio::test]
async fn replacement_retains_exact_recipe_layout_and_transitive_code_bytes() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (recipe, code, helper) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let helper_draft = draft(
        helper,
        22,
        json!({"content":"result = inputs[\"value\"]"}),
        &[],
    );
    let code_draft = draft(
        code,
        22,
        json!({"content":"result = inputs[\"message\"]", "includes":[helper]}),
        &[helper],
    );
    let recipe_draft = draft(
        recipe,
        21,
        json!({"variants":[{"variant_key":"reply","step_link":"0:1-0:E","variable_patterns":[]}],
        "step_descriptions":[{"desc_idx":0,"steps":[{"stepnumber":1,"knowledge":"orchestrator","type":"component","include":[code]}]}]}),
        &[code],
    );
    let helper_ref = store.stage(&helper_draft, 0).await.unwrap();
    let code_ref = store.stage(&code_draft, 0).await.unwrap();
    let recipe_ref = store.stage(&recipe_draft, 0).await.unwrap();
    let refs = [recipe_ref, code_ref, helper_ref];
    let first = store.read_exact(&[recipe], &refs).await.unwrap();
    let replacement = draft(
        code,
        22,
        json!({"content":"result = 'replacement'", "includes":[helper]}),
        &[helper],
    );
    let newer_code = store.stage(&replacement, 1).await.unwrap();
    assert_eq!(newer_code.version, 2);
    assert_ne!(newer_code.checksum, code_ref.checksum);
    let resumed = store.read_exact(&[recipe], &refs).await.unwrap();
    for snapshot in [&first, &resumed] {
        assert_eq!(
            snapshot.revisions()[&code].draft().exact_bytes(),
            code_draft.exact_bytes()
        );
        assert_eq!(
            snapshot.revisions()[&recipe].draft().exact_bytes(),
            recipe_draft.exact_bytes()
        );
        assert_eq!(snapshot.revisions()[&helper].reference(), helper_ref);
    }
    let new_selection = store
        .read_exact(&[recipe], &[recipe_ref, newer_code, helper_ref])
        .await
        .unwrap();
    assert_eq!(
        new_selection.revisions()[&code].draft().exact_bytes(),
        replacement.exact_bytes()
    );
    // Review evidence has foreign keys into retained revisions. PostgreSQL
    // rejects plain TRUNCATE before running statement triggers; CASCADE reaches
    // the immutable trigger and must also leave every old/new revision intact.
    for (sql, expected_code) in [
        (
            "UPDATE reborn_component_revisions SET revision_bytes=revision_bytes",
            "23514",
        ),
        ("DELETE FROM reborn_component_revisions", "23514"),
        ("TRUNCATE reborn_component_revisions", "0A000"),
        ("TRUNCATE reborn_component_revisions CASCADE", "23514"),
    ] {
        let error = rig
            .pool
            .get()
            .await
            .unwrap()
            .execute(sql, &[])
            .await
            .unwrap_err();
        assert_eq!(error.code().unwrap().code(), expected_code);
        let retained = store.read_exact(&[recipe], &refs).await.unwrap();
        assert_eq!(
            retained.revisions()[&code].draft().exact_bytes(),
            code_draft.exact_bytes()
        );
        let retained_new = store
            .read_exact(&[recipe], &[recipe_ref, newer_code, helper_ref])
            .await
            .unwrap();
        assert_eq!(
            retained_new.revisions()[&code].draft().exact_bytes(),
            replacement.exact_bytes()
        );
    }
}

#[tokio::test]
async fn simultaneous_edits_conflict_and_failed_transaction_does_not_consume_a_version() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let id = Uuid::new_v4();
    let initial = draft(id, 22, json!({"content":"result = 0"}), &[]);
    let first = store.stage(&initial, 0).await.unwrap();
    let left = draft(id, 22, json!({"content":"result = 1"}), &[]);
    let right = draft(id, 22, json!({"content":"result = 2"}), &[]);
    let (left_result, right_result) = tokio::join!(store.stage(&left, 1), store.stage(&right, 1));
    let winner = match (left_result, right_result) {
        (Ok(reference), Err(RevisionStoreError::Conflict))
        | (Err(RevisionStoreError::Conflict), Ok(reference)) => reference,
        _ => panic!("exactly one edit must win"),
    };
    assert_eq!(winner.version, 2);
    assert!(store.read_exact(&[id], &[first]).await.is_ok());
    assert!(store.read_exact(&[id], &[winner]).await.is_ok());
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let rolled_back = PgComponentRevisionStore::stage_in_transaction(&tx, &initial, 2)
        .await
        .unwrap();
    assert_eq!(rolled_back.version, 3);
    tx.rollback().await.unwrap();
    assert!(matches!(
        store.read_exact(&[id], &[rolled_back]).await,
        Err(RevisionStoreError::Integrity)
    ));
    let next = store.stage(&right, 2).await.unwrap();
    assert_eq!(next.version, 3);
    let wrong_class = draft(id, 21, json!({}), &[]);
    assert!(matches!(
        store.stage(&wrong_class, 3).await,
        Err(RevisionStoreError::Conflict)
    ));
}

#[tokio::test]
async fn exact_snapshot_rejects_missing_extra_cycles_and_wrong_checksums() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, dep, extra) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let root_ref = store
        .stage(&draft(root, 21, json!({}), &[dep]), 0)
        .await
        .unwrap();
    let dep_ref = store
        .stage(&draft(dep, 22, json!({"content":"result = None"}), &[]), 0)
        .await
        .unwrap();
    let extra_ref = store
        .stage(
            &draft(extra, 22, json!({"content":"result = None"}), &[]),
            0,
        )
        .await
        .unwrap();
    assert!(matches!(
        store.read_exact(&[root], &[root_ref]).await,
        Err(RevisionStoreError::Invalid(RevisionError::Invalid(
            "missing dependency revision"
        )))
    ));
    assert!(
        store
            .read_exact(&[root], &[root_ref, dep_ref, extra_ref])
            .await
            .is_err()
    );
    let mut corrupt = dep_ref;
    corrupt.checksum[0] ^= 1;
    assert!(matches!(
        store.read_exact(&[root], &[root_ref, corrupt]).await,
        Err(RevisionStoreError::Integrity)
    ));
    corrupt = dep_ref;
    corrupt.class_code = 0;
    assert!(matches!(
        store.read_exact(&[root], &[root_ref, corrupt]).await,
        Err(RevisionStoreError::Integrity)
    ));
    let cycle = store
        .stage(
            &draft(dep, 22, json!({"content":"result = None"}), &[root]),
            1,
        )
        .await
        .unwrap();
    assert!(matches!(
        store.read_exact(&[root], &[root_ref, cycle]).await,
        Err(RevisionStoreError::Invalid(RevisionError::Invalid(
            "cyclic dependency graph"
        )))
    ));
    // The original pinned graph remains intact after the incompatible draft.
    assert!(
        store
            .read_exact(&[root], &[root_ref, dep_ref])
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn skill_revision_retains_exact_association_and_requires_correct_component_classes() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (skill, python, binding, tool) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let association = json!({"format":"skill-association/1","skill_uuid":skill,
        "python_code_uuid":python,"tool_skill_uuid":binding,"tool_uuid":tool,
        "callable":"host.post_reply","inputs":{"answer":{"type":"string","required":true,"checks":[]}},
        "arguments":{"answer":"answer"},"code_arguments":{},
        "result":{"type":"string"},"failure":{"action":"stop","max_attempts":1,
        "idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}})
    .to_string();
    let bytes = json!({"format":"component-revision/1","uuid":skill,"class_code":1,
        "document":{"body":"Post a reply."},"dependencies":[python,binding,tool],"association":association}).to_string();
    let skill_draft = ComponentRevisionDraft::from_json(&bytes).unwrap();
    assert_eq!(
        skill_draft.association().unwrap().exact_bytes(),
        association
    );
    let skill_ref = store.stage(&skill_draft, 0).await.unwrap();
    let python_ref = store
        .stage(
            &draft(
                python,
                22,
                json!({"content":"result = host.post_reply(answer=inputs['answer'])"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let binding_ref = store
        .stage(
            &draft(binding, 13, json!({"name":"ts-host-post-reply"}), &[]),
            0,
        )
        .await
        .unwrap();
    let tool_ref = store
        .stage(&draft(tool, 0, json!({"name":"post_reply"}), &[]), 0)
        .await
        .unwrap();
    let snapshot = store
        .read_exact(&[skill], &[skill_ref, python_ref, binding_ref, tool_ref])
        .await
        .unwrap();
    assert_eq!(
        snapshot.revisions()[&skill]
            .draft()
            .association()
            .unwrap()
            .exact_bytes(),
        association
    );
    // Wrong class is checked from actual retained rows, not just caller refs.
    let wrong_python = Uuid::new_v4();
    let mut bad: Value = serde_json::from_str(&bytes).unwrap();
    let mut wrong_association: Value = serde_json::from_str(&association).unwrap();
    wrong_association["python_code_uuid"] = json!(wrong_python);
    bad["association"] = json!(wrong_association.to_string());
    bad["dependencies"] = json!([wrong_python, binding, tool]);
    let bad_ref = store
        .stage(
            &ComponentRevisionDraft::from_json(&bad.to_string()).unwrap(),
            1,
        )
        .await
        .unwrap();
    let wrong_ref = store
        .stage(
            &draft(
                wrong_python,
                17,
                json!({"content":"Prose is not Python"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    assert!(
        store
            .read_exact(&[skill], &[bad_ref, wrong_ref, binding_ref, tool_ref])
            .await
            .is_err()
    );
    // No association/evidence/activation is synthesized by retention.
    let missing = json!({"format":"component-revision/1","uuid":Uuid::new_v4(),"class_code":1,"document":{},"dependencies":[],"association":null});
    assert!(ComponentRevisionDraft::from_json(&missing.to_string()).is_err());
    let duplicate = bytes.replacen(
        "\"format\":",
        "\"format\":\"component-revision/1\",\"format\":",
        1,
    );
    assert!(ComponentRevisionDraft::from_json(&duplicate).is_err());
}

#[tokio::test]
async fn exact_revision_reads_share_the_callers_consistent_selection_snapshot() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, code) = (Uuid::new_v4(), Uuid::new_v4());
    let original = store
        .stage(&draft(code, 22, json!({"content":"result = 1"}), &[]), 0)
        .await
        .unwrap();
    let recipe = store
        .stage(&draft(root, 21, json!({}), &[code]), 0)
        .await
        .unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let weak = client.transaction().await.unwrap();
    assert!(matches!(
        PgComponentRevisionStore::read_exact_in_transaction(&weak, &[root], &[recipe, original])
            .await,
        Err(RevisionStoreError::SnapshotIsolation)
    ));
    weak.rollback().await.unwrap();
    let selected = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let head = selected
        .query_one(
            "SELECT last_version FROM reborn_component_revision_heads WHERE component_id=$1",
            &[&code],
        )
        .await
        .unwrap();
    assert_eq!(head.get::<_, i64>(0), 1);
    let replacement = store
        .stage(&draft(code, 22, json!({"content":"result = 2"}), &[]), 1)
        .await
        .unwrap();
    let retained = PgComponentRevisionStore::read_exact_in_transaction(
        &selected,
        &[root],
        &[recipe, original],
    )
    .await
    .unwrap();
    assert_eq!(retained.revisions()[&code].reference(), original);
    assert_eq!(
        retained.revisions()[&code].draft().document()["content"],
        "result = 1"
    );
    assert!(matches!(
        PgComponentRevisionStore::read_exact_in_transaction(
            &selected,
            &[root],
            &[recipe, replacement]
        )
        .await,
        Err(RevisionStoreError::Integrity)
    ));
    let head = selected
        .query_one(
            "SELECT last_version FROM reborn_component_revision_heads WHERE component_id=$1",
            &[&code],
        )
        .await
        .unwrap();
    assert_eq!(head.get::<_, i64>(0), 1);
    selected.commit().await.unwrap();
    let next = store
        .read_exact(&[root], &[recipe, replacement])
        .await
        .unwrap();
    assert_eq!(next.revisions()[&code].reference(), replacement);
    // A retained old selection remains readable after the selecting transaction ends.
    assert!(store.read_exact(&[root], &[recipe, original]).await.is_ok());
}
