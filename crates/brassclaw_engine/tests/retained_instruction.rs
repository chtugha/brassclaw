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
        Arc::new(
            store
                .read_exact(
                    &[recipe],
                    &refs,
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
                .await
                .unwrap(),
        ),
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
        Arc::new(
            store
                .read_exact(
                    &[recipe],
                    &refs,
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
                .await
                .unwrap(),
        ),
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
                .read_exact(
                    &[recipe],
                    &[newer, new_first, second_ref],
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
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
            .read_exact(
                &[recipe],
                &[valid_ref, code_ref, prose_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
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
                .read_exact(
                    &[recipe],
                    &[revision, code_ref, prose_ref],
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
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
            .read_exact(
                &[recipe],
                &[valid_ref, code_ref, prose_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
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
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
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
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
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
            .read_exact(
                &[recipe],
                &[ambiguous_ref, first_ref, second_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
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

#[tokio::test]
async fn retained_eligibility_filters_non_catalogue_and_pending_rows_before_ranking() {
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentResolution, IntentScope, IntentSource, IntentSystemError,
        RetainedIntentEligibility, resolve_catalogue_intent_in_transaction, seed_intent_input,
    };
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let scope = IntentScope {
        tenant_id: "eligible-catalogue".into(),
        user_id: "operator".into(),
        agent_id: "agent".into(),
        project_id: "project".into(),
    };
    let (first, second, recipe, excluded) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let first_ref = store
        .stage(&draft(first, 22, json!({"content":"result = 1"}), &[]), 0)
        .await
        .unwrap();
    let second_ref = store
        .stage(&draft(second, 22, json!({"content":"result = 2"}), &[]), 0)
        .await
        .unwrap();
    let link = "2:1-2:E+0:1-0:E";
    let chosen = store
        .stage(
            &draft(
                recipe,
                21,
                recipe_document(first, second, link),
                &[first, second],
            ),
            0,
        )
        .await
        .unwrap();
    let other = store
        .stage(
            &draft(
                excluded,
                21,
                recipe_document(first, second, link),
                &[first, second],
            ),
            0,
        )
        .await
        .unwrap();
    for id in [recipe, excluded] {
        seed_intent_input(
            &rig.pool,
            &scope,
            "ordered input",
            InputClass::Partial,
            id,
            21,
            IntentSource::Seeded,
            Some(link),
        )
        .await
        .unwrap();
    }
    // This is routing-fixture membership, not fabricated component approval.
    let graph = Arc::new(
        store
            .read_exact(
                &[recipe],
                &[chosen, first_ref, second_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let instruction =
        compile_retained_recipe(graph, recipe, "selected", WorkflowClass::Deterministic).unwrap();
    let eligible = RetainedIntentEligibility::from_instructions(&[&instruction]).unwrap();
    let empty = RetainedIntentEligibility::from_instructions(&[]).unwrap();
    // Repeated selected variants share one immutable graph. More than the old
    // instruction/visit counters must not invalidate this one eligible workflow.
    let repeated = vec![&instruction; 22_000];
    let repeated = RetainedIntentEligibility::from_instructions(&repeated).unwrap();
    let mut client = rig.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE reborn_intent_inputs SET score=100 WHERE component_id=$1",
            &[&excluded],
        )
        .await
        .unwrap();
    async fn lookup(
        pool: &brassclaw_pg::PgPool,
        scope: &IntentScope,
        eligible: &RetainedIntentEligibility,
    ) -> IntentResolution {
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let matched =
            resolve_catalogue_intent_in_transaction(&tx, scope, "ordered input", eligible)
                .await
                .unwrap();
        tx.commit().await.unwrap();
        matched
    }
    assert!(
        matches!(lookup(&rig.pool,&scope,&eligible).await,IntentResolution::Match { component_id,.. } if component_id==recipe)
    );
    assert!(
        matches!(lookup(&rig.pool,&scope,&repeated).await,IntentResolution::Match { component_id,.. } if component_id==recipe)
    );
    assert!(matches!(
        lookup(&rig.pool, &scope, &empty).await,
        IntentResolution::NoMatch
    ));
    // A stale link under the same UUID is outside this exact selected variant.
    client
        .execute(
            "UPDATE reborn_intent_inputs SET step_link='0:1-0:E+2:1-2:E' WHERE component_id=$1",
            &[&recipe],
        )
        .await
        .unwrap();
    assert!(matches!(
        lookup(&rig.pool, &scope, &eligible).await,
        IntentResolution::NoMatch
    ));
    client
        .execute(
            "UPDATE reborn_intent_inputs SET step_link=$2,needs_review=true WHERE component_id=$1",
            &[&recipe, &link],
        )
        .await
        .unwrap();
    assert!(matches!(
        lookup(&rig.pool, &scope, &eligible).await,
        IntentResolution::NoMatch
    ));
    client
        .execute(
            "UPDATE reborn_intent_inputs SET needs_review=false WHERE component_id=$1",
            &[&recipe],
        )
        .await
        .unwrap();
    assert!(
        matches!(lookup(&rig.pool,&scope,&eligible).await,IntentResolution::Match { component_id,.. } if component_id==recipe)
    );
    // Two revisions of one stable identity cannot form a coherent generation.
    let replacement = store
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
    let graph = Arc::new(
        store
            .read_exact(
                &[recipe],
                &[replacement, first_ref, second_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let newer =
        compile_retained_recipe(graph, recipe, "selected", WorkflowClass::Deterministic).unwrap();
    assert!(matches!(
        RetainedIntentEligibility::from_instructions(&[&instruction, &newer]),
        Err(IntentSystemError::InvalidEligibility)
    ));
    // Matching/SQL failure remains a real error, even for an empty eligible set.
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    tx.batch_execute("ALTER TABLE reborn_intent_inputs RENAME TO temporarily_unavailable_intents")
        .await
        .unwrap();
    assert!(matches!(
        resolve_catalogue_intent_in_transaction(&tx, &scope, "ordered input", &empty).await,
        Err(IntentSystemError::Db(_))
    ));
    tx.rollback().await.unwrap();
    // Keep this truly staged second graph available without certifying approval.
    assert!(
        store
            .read_exact(
                &[excluded],
                &[other, first_ref, second_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph()
            )
            .await
            .unwrap()
            .roots()
            .contains(&excluded)
    );
}

#[test]
fn retained_eligibility_accepts_complete_large_exact_graphs() {
    use brassclaw_engine::memory::intent_system::RetainedIntentEligibility;
    use brassclaw_skills::component_revision::RetainedComponentSnapshot;
    let recipe = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut ids = vec![first, second];
    ids.extend((0..4_100).map(|_| Uuid::new_v4()));
    let mut revisions = ids
        .iter()
        .map(|id| {
            draft(*id, 22, json!({"content":"result = 1"}), &[])
                .at_version(1)
                .unwrap()
        })
        .collect::<Vec<_>>();
    revisions.push(
        draft(
            recipe,
            21,
            recipe_document(first, second, "2:1-2:E+0:1-0:E"),
            &ids,
        )
        .at_version(1)
        .unwrap(),
    );
    let graph = Arc::new(RetainedComponentSnapshot::new(&[recipe], revisions).unwrap());
    assert!(graph.revisions().len() > 4_096);
    let instruction =
        compile_retained_recipe(graph, recipe, "selected", WorkflowClass::Deterministic).unwrap();
    // Validated in-memory revision data tests this neutral compiler/helper only;
    // it is not a catalogue activation or fabricated component approval.
    RetainedIntentEligibility::from_instructions(&[&instruction]).unwrap();
}

#[tokio::test]
async fn retained_disambiguation_returns_all_eligible_workflows_without_alias_duplicates() {
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentResolution, IntentScope, IntentSource, RetainedIntentEligibility,
        resolve_catalogue_intent_in_transaction, seed_intent_input,
    };
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let scope = IntentScope {
        tenant_id: "complete-disambiguation".into(),
        user_id: "operator".into(),
        agent_id: "agent".into(),
        project_id: "project".into(),
    };
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let first_ref = store
        .stage(&draft(first, 22, json!({"content":"result = 1"}), &[]), 0)
        .await
        .unwrap();
    let second_ref = store
        .stage(&draft(second, 22, json!({"content":"result = 2"}), &[]), 0)
        .await
        .unwrap();
    let mut instructions = Vec::new();
    let mut expected = std::collections::BTreeSet::new();
    for index in 0..7_i32 {
        let id = Uuid::new_v4();
        let link = "2:1-2:E+0:1-0:E";
        let mut document = recipe_document(first, second, link);
        document["variants"][0]["intent_examples"] = json!(["ordered input", "ordered %"]);
        let selected = store
            .stage(&draft(id, 21, document, &[first, second]), 0)
            .await
            .unwrap();
        let graph = Arc::new(
            store
                .read_exact(
                    &[id],
                    &[selected, first_ref, second_ref],
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
                .await
                .unwrap(),
        );
        instructions.push(
            compile_retained_recipe(graph, id, "selected", WorkflowClass::Deterministic).unwrap(),
        );
        // This fixture establishes exact routing membership, not approval or
        // activation. Both aliases identify the same retained workflow.
        for text in ["ordered input", "ordered %"] {
            seed_intent_input(
                &rig.pool,
                &scope,
                text,
                InputClass::Partial,
                id,
                21,
                IntentSource::Seeded,
                Some(link),
            )
            .await
            .unwrap();
        }
        let score = if index < 6 { 7 - index % 3 } else { 4 };
        rig.pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE reborn_intent_inputs SET score=$2 WHERE component_id=$1",
                &[&id, &score],
            )
            .await
            .unwrap();
        if index < 6 {
            expected.insert(id);
        }
    }
    let references: Vec<_> = instructions.iter().collect();
    let eligible = RetainedIntentEligibility::from_instructions(&references).unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let result = resolve_catalogue_intent_in_transaction(&tx, &scope, "ordered input", &eligible)
        .await
        .unwrap();
    let IntentResolution::Disambiguation { candidates } = result else {
        panic!("expected complete ambiguity");
    };
    assert_eq!(candidates.len(), 6);
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.component_id)
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert!(
        candidates
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score)
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.component_class_code == 21
                && candidate.step_link.as_deref() == Some("2:1-2:E+0:1-0:E"))
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn model_input_binding_is_pinned_with_exact_recipe_and_never_added_to_legacy_selection() {
    use brassclaw_skills::recipe_model_input::RecipeModelInput;
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_ref = store
        .stage(
            &draft(first, 22, json!({"content":"result = 'first'"}), &[]),
            0,
        )
        .await
        .unwrap();
    let second_ref = store
        .stage(
            &draft(second, 22, json!({"content":"result = 'second'"}), &[]),
            0,
        )
        .await
        .unwrap();
    let mut document = recipe_document(first, second, "2:1-2:E+0:1-0:E");
    document["input_layouts"] = json!({"selected":{"format":"recipe-input-layout/1","task_inputs":{},"steps":{"2:1":{},"0:1":{}}}});
    let legacy_ref = store
        .stage(&draft(root, 21, document.clone(), &[first, second]), 0)
        .await
        .unwrap();
    let legacy_snapshot = Arc::new(
        store
            .read_exact(
                &[root],
                &[legacy_ref, first_ref, second_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let legacy = compile_retained_recipe(
        legacy_snapshot.clone(),
        root,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let old = legacy.retained_selection().unwrap();
    let old_bytes = old.exact_bytes().to_owned();
    assert_eq!(legacy.model_input(), &RecipeModelInput::Default);
    assert!(
        serde_json::from_str::<Value>(&old_bytes)
            .unwrap()
            .get("model_input_binding")
            .is_none()
    );
    document["model_input_bindings"] = json!({"format":"recipe-model-input-bindings/1",
        "variants":{"selected":{"selection":"selected","profile":"defensive"}}});
    let selected_ref = store
        .stage(&draft(root, 21, document.clone(), &[first, second]), 1)
        .await
        .unwrap();
    let selected_snapshot = Arc::new(
        store
            .read_exact(
                &[root],
                &[selected_ref, first_ref, second_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let instruction = compile_retained_recipe(
        selected_snapshot.clone(),
        root,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    assert_eq!(
        instruction.model_input(),
        &RecipeModelInput::Selected {
            profile: "defensive".into()
        }
    );
    let selected = instruction.retained_selection().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(selected.exact_bytes()).unwrap()["model_input_binding"],
        json!({"format":"recipe-model-input-selection/1","binding":{"selection":"selected","profile":"defensive"}})
    );
    // A later immutable Recipe revision cannot replace either original selection.
    document["model_input_bindings"]["variants"]["selected"] =
        json!({"selection":"prefix_free_workflow","purpose":"prefix_generation"});
    store
        .stage(&draft(root, 21, document, &[first, second]), 2)
        .await
        .unwrap();
    let resumed = compile_retained_recipe(
        selected_snapshot,
        root,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    assert_eq!(
        resumed.retained_selection().unwrap().exact_bytes(),
        selected.exact_bytes()
    );
    let old_resumed = compile_retained_recipe(
        legacy_snapshot,
        root,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    assert_eq!(
        old_resumed.retained_selection().unwrap().exact_bytes(),
        old_bytes
    );
    assert_ne!(old.checksum(), selected.checksum());
}

/// Actual retained graph and source inspector, including dependency-first symbol
/// resolution. This is an unapproved component fixture, never activation proof.
#[tokio::test]
async fn bootstrap_prefill_uses_retained_budget_export_instead_of_parallel_arithmetic() {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::retained_inputs::prepare_retained_unbound_program,
    };
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (recipe, answer, budget) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let answer_draft =
        brassclaw_skills::prefix_bootstrap_components::bootstrap_answer_draft(answer, budget)
            .unwrap();
    let budget_draft =
        brassclaw_skills::prefix_compiler_components::model_budget_draft(budget).unwrap();
    let recipe_draft = draft(
        recipe,
        21,
        json!({
            "variants":[{"variant_key":"selected", "step_link":"0:1-0:E", "intent_examples":["bootstrap candidate"], "variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Check captured bootstrap option","yaml_source":"", "steps":[{
                "stepnumber":1,"knowledge":"orchestrator","goal":"Check candidate without importing or approving it", "content":"","type":"component","include":[answer]
            }]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1",
                "task_inputs":answer_draft.document()["input_contract"],
                "steps":{"0:1":{"phase":{"kind":"task_input","reference":"{{vars.phase}}"}, "data_bytes":{"kind":"task_input","reference":"{{vars.data_bytes}}"}}}}},
            "invocations":{"selected":{"0:1":"bootstrap_answer_stage"}}
        }),
        &[answer],
    );
    let mut references = Vec::new();
    for component in [&budget_draft, &answer_draft, &recipe_draft] {
        references.push(store.stage(component, 0).await.unwrap());
    }
    let snapshot = Arc::new(
        store
            .read_exact(
                &[recipe],
                &references,
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let instruction =
        compile_retained_recipe(snapshot, recipe, "selected", WorkflowClass::Deterministic)
            .unwrap();
    let program = prepare_retained_unbound_program(instruction).unwrap();
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Unbound(Arc::new(program)),
        &brassclaw_monty_host::process::installed_worker().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(inspected.preload_order(), &[budget, answer]);
    assert_eq!(
        inspected.invocation("0:1"),
        Some("result = bootstrap_answer_stage(inputs=inputs)")
    );
}
