//! Real PostgreSQL -> retained Recipe -> actual IBS. This checks pinned workflow
//! compilation, not activation, behavioral approval or production execution.
#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_engine::memory::retained_instruction::{
    RetainedInstructionError, WorkflowClass, compile_matched_retained_recipe,
    compile_retained_recipe,
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "../../brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

fn draft(id: Uuid, class: i32, document: Value, dependencies: &[Uuid]) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,
        "class_code":class,"document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )
    .unwrap()
}
fn recipe_document(first: Uuid, second: Uuid, link: &str) -> Value {
    json!({"variants":[{"variant_key":"selected","description":"Retain this exact layout",
        "step_link":link,"intent_examples":["ordered input"],"variable_patterns":[{"name":"value","pattern":null,"description":null}]}],
        "step_descriptions":[
            {"desc_idx":2,"label":"first","yaml_source":"","steps":[{"stepnumber":1,"knowledge":"orchestrator","goal":"first","content":"","type":"component","include":[first]}]},
            {"desc_idx":0,"label":"second","yaml_source":"","steps":[{"stepnumber":1,"knowledge":"orchestrator","goal":"second","content":"","type":"component","include":[second]}]}]})
}

#[tokio::test]
async fn paused_selection_keeps_recipe_variant_step_link_order_and_exact_dependencies() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (recipe, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_ref = store
        .stage(
            &draft(
                first,
                22,
                json!({"content":"result = inputs[\"value\"]"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let second_ref = store
        .stage(
            &draft(
                second,
                22,
                json!({"content":"result = inputs[\"previous\"]"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let old_recipe = store
        .stage(
            &draft(
                recipe,
                21,
                recipe_document(first, second, "2:1-2:E+0:1-0:E"),
                &[first, second],
            ),
            0,
        )
        .await
        .unwrap();
    let refs = [old_recipe, first_ref, second_ref];
    let before = compile_retained_recipe(
        Arc::new(store.read_exact(&[recipe], &refs).await.unwrap()),
        recipe,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let newer = store
        .stage(
            &draft(
                recipe,
                21,
                recipe_document(first, second, "0:1-0:E+2:1-2:E"),
                &[first, second],
            ),
            1,
        )
        .await
        .unwrap();
    let new_first = store
        .stage(
            &draft(first, 22, json!({"content":"result = 'new version'"}), &[]),
            1,
        )
        .await
        .unwrap();
    let resumed = compile_retained_recipe(
        Arc::new(store.read_exact(&[recipe], &refs).await.unwrap()),
        recipe,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    for selected in [&before, &resumed] {
        assert_eq!(selected.recipe(), old_recipe);
        assert_eq!(
            selected.variant().step_link.as_deref(),
            Some("2:1-2:E+0:1-0:E")
        );
        assert_eq!(selected.ordered().step_order(), ["2:1", "0:1"]);
        assert_eq!(
            selected.ordered().instruction().variable_patterns[0].name,
            "value"
        );
        assert_eq!(
            selected.snapshot().revisions()[&first].reference(),
            first_ref
        );
        assert!(!selected.ordered().instruction().llm_call_required);
    }
    let next = compile_retained_recipe(
        Arc::new(
            store
                .read_exact(&[recipe], &[newer, new_first, second_ref])
                .await
                .unwrap(),
        ),
        recipe,
        "selected",
        WorkflowClass::RequiresModel,
    )
    .unwrap();
    assert_eq!(next.ordered().step_order(), ["0:1", "2:1"]);
    assert_eq!(next.snapshot().revisions()[&first].reference(), new_first);
    assert!(next.ordered().instruction().llm_call_required);
}

#[tokio::test]
async fn retained_recipe_rejects_ambiguity_prose_execution_and_unsupported_workflow_fields() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (recipe, code, prose) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let code_ref = store
        .stage(&draft(code, 22, json!({"content":"result = None"}), &[]), 0)
        .await
        .unwrap();
    let prose_ref = store
        .stage(
            &draft(
                prose,
                17,
                json!({"content":"Instructions are not Python"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let valid = recipe_document(code, prose, "2:1-2:E");
    let valid_ref = store
        .stage(&draft(recipe, 21, valid.clone(), &[code, prose]), 0)
        .await
        .unwrap();
    let snapshot = Arc::new(
        store
            .read_exact(&[recipe], &[valid_ref, code_ref, prose_ref])
            .await
            .unwrap(),
    );
    assert!(matches!(
        compile_retained_recipe(
            snapshot.clone(),
            recipe,
            "absent",
            WorkflowClass::Deterministic
        ),
        Err(RetainedInstructionError::Invalid(
            "selected variant does not belong to Recipe revision"
        ))
    ));
    assert!(
        compile_retained_recipe(snapshot, recipe, "selected", WorkflowClass::Deterministic).is_ok()
    );
    let mut cases = Vec::new();
    let mut bad = valid.clone();
    bad["variants"][0]["step_link"] = json!("0:1-0:E");
    cases.push(bad);
    let mut bad = valid.clone();
    bad["variants"]
        .as_array_mut()
        .unwrap()
        .push(valid["variants"][0].clone());
    cases.push(bad);
    let mut bad = valid.clone();
    bad["step_descriptions"][0]["steps"][0]["include"] = json!([code, prose]);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["step_descriptions"][0]["steps"][0]["hidden_execution"] = json!(true);
    cases.push(bad);
    let mut bad = valid;
    bad["variants"][0]["variable_patterns"][0]["hidden_transform"] = json!("eval");
    cases.push(bad);
    for (previous, bad) in (1..).zip(cases) {
        let revision = store
            .stage(&draft(recipe, 21, bad, &[code, prose]), previous)
            .await
            .unwrap();
        let snapshot = Arc::new(
            store
                .read_exact(&[recipe], &[revision, code_ref, prose_ref])
                .await
                .unwrap(),
        );
        assert!(
            compile_retained_recipe(snapshot, recipe, "selected", WorkflowClass::Deterministic)
                .is_err()
        );
    }
    // Invalid replacement drafts cannot alter the old workflow's compilation.
    let old = Arc::new(
        store
            .read_exact(&[recipe], &[valid_ref, code_ref, prose_ref])
            .await
            .unwrap(),
    );
    assert!(compile_retained_recipe(old, recipe, "selected", WorkflowClass::Deterministic).is_ok());
}

#[tokio::test]
async fn actual_matching_and_ibs_share_one_view_and_reject_variant_identity_loss() {
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentResolution, IntentScope, IntentSource, resolve_intent_in_transaction,
        seed_intent_input,
    };
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let scope = IntentScope {
        tenant_id: "retained-match".into(),
        user_id: "operator".into(),
        agent_id: "agent".into(),
        project_id: "project".into(),
    };
    let (recipe, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_ref = store
        .stage(
            &draft(
                first,
                22,
                json!({"content":"result = inputs['value']"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let second_ref = store
        .stage(
            &draft(
                second,
                22,
                json!({"content":"result = inputs['previous']"}),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let old_document = recipe_document(first, second, "2:1-2:E+0:1-0:E");
    let old = store
        .stage(
            &draft(recipe, 21, old_document.clone(), &[first, second]),
            0,
        )
        .await
        .unwrap();
    seed_intent_input(
        &rig.pool,
        &scope,
        "ordered input",
        InputClass::Partial,
        recipe,
        21,
        IntentSource::Seeded,
        Some("2:1-2:E+0:1-0:E"),
    )
    .await
    .unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let matched = resolve_intent_in_transaction(&tx, &scope, "ordered input")
        .await
        .unwrap();
    let newer = store
        .stage(
            &draft(
                recipe,
                21,
                recipe_document(first, second, "0:1-0:E+2:1-2:E"),
                &[first, second],
            ),
            1,
        )
        .await
        .unwrap();
    seed_intent_input(
        &rig.pool,
        &scope,
        "ordered input",
        InputClass::Partial,
        recipe,
        21,
        IntentSource::Seeded,
        Some("0:1-0:E+2:1-2:E"),
    )
    .await
    .unwrap();
    let snapshot = Arc::new(
        PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[recipe],
            &[old, first_ref, second_ref],
        )
        .await
        .unwrap(),
    );
    let instruction =
        compile_matched_retained_recipe(snapshot.clone(), &matched, WorkflowClass::Deterministic)
            .unwrap();
    assert_eq!(instruction.recipe(), old);
    assert_eq!(instruction.variant().variant_key, "selected");
    assert_eq!(instruction.ordered().step_order(), ["2:1", "0:1"]);
    tx.commit().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let next_match = resolve_intent_in_transaction(&tx, &scope, "ordered input")
        .await
        .unwrap();
    let next = Arc::new(
        PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[recipe],
            &[newer, first_ref, second_ref],
        )
        .await
        .unwrap(),
    );
    let next_instruction =
        compile_matched_retained_recipe(next.clone(), &next_match, WorkflowClass::RequiresModel)
            .unwrap();
    assert_eq!(next_instruction.recipe(), newer);
    assert_eq!(next_instruction.ordered().step_order(), ["0:1", "2:1"]);
    assert!(next_instruction.ordered().instruction().llm_call_required);
    assert!(
        compile_matched_retained_recipe(next, &matched, WorkflowClass::Deterministic).is_err(),
        "matching and assembly from different views must not silently select a variant"
    );
    let absent = resolve_intent_in_transaction(&tx, &scope, "absent")
        .await
        .unwrap();
    assert!(matches!(absent, IntentResolution::NoMatch));
    assert!(
        compile_matched_retained_recipe(snapshot.clone(), &absent, WorkflowClass::Deterministic)
            .is_err()
    );
    tx.commit().await.unwrap();
    // Same link with a different embedded variant/layout remains ambiguous.
    // A duplicate variant key is already independently rejected by strict IBS.
    let mut ambiguous = old_document;
    let mut other = ambiguous["variants"][0].clone();
    other["variant_key"] = json!("other-layout");
    other["variable_patterns"][0]["name"] = json!("different_value");
    ambiguous["variants"].as_array_mut().unwrap().push(other);
    let ambiguous_ref = store
        .stage(&draft(recipe, 21, ambiguous, &[first, second]), 2)
        .await
        .unwrap();
    let ambiguous_snapshot = Arc::new(
        store
            .read_exact(&[recipe], &[ambiguous_ref, first_ref, second_ref])
            .await
            .unwrap(),
    );
    assert!(matches!(
        compile_matched_retained_recipe(ambiguous_snapshot, &matched, WorkflowClass::Deterministic),
        Err(RetainedInstructionError::Invalid(
            "matched intent identifies multiple retained Recipe variants"
        ))
    ));
    assert!(
        compile_matched_retained_recipe(snapshot, &matched, WorkflowClass::Deterministic).is_ok(),
        "later conflicting revisions cannot invalidate old exact selection"
    );
}
