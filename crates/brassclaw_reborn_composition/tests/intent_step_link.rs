//! Real native PostgreSQL intent selection, disambiguation and scoring.
//! Fixtures test the legacy scoped lookup, not an activated v3 catalogue.
//! Initialization errors fail the tests; no unavailable-container success path.

#![cfg(feature = "skills-db")]

use brassclaw_engine::memory::intent_system::{
    InputClass, IntentResolution, IntentScope, IntentSource, record_disambiguation_choice,
    resolve_intent, seed_intent_input,
};
use uuid::Uuid;

#[path = "../../brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

/// A fresh, isolated scope per test (unique tenant) so parallel tests never
/// collide on `reborn_intent_inputs` / `reborn_actions` rows or the per-scope
/// score rate-limit bucket (SEC-05: 50 increments/hour).
fn unique_scope() -> IntentScope {
    IntentScope {
        tenant_id: format!("t-{}", Uuid::new_v4()),
        user_id: "u".to_string(),
        agent_id: "a".to_string(),
        project_id: "p".to_string(),
    }
}

/// A unique sentence-class query (≥5 whitespace tokens, no terminal
/// punctuation) so `classify_query` → `Sentence` and `match_order` = `[3, 2,
/// 1]`; the seeded `input_class = Sentence` is therefore in the `ANY($6)` set.
/// The seeded `input_text` MUST char-for-char equal the query passed to
/// `resolve_intent`.
fn unique_sentence_query() -> String {
    format!("trigger the recipe intent match {}", Uuid::new_v4())
}

/// Insert a minimal class-16 `reborn_actions` row: only the NOT-NULL-no-default
/// columns (scope tuple + `name` + `description`) are supplied; every other
/// column has a default. UUID-derived name keeps parallel runs off the
/// `UNIQUE(scope, name)` constraint. Returns the assigned row id.
async fn insert_action_row(
    pool: &deadpool_postgres::Pool,
    scope: &IntentScope,
    name: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_actions
                 (id, tenant_id, user_id, agent_id, project_id, name, description)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"daily sync action description",
            ],
        )
        .await
        .expect("insert reborn_actions");
    id
}

/// Fetch the `reborn_intent_inputs.id` for a seeded (scope, input_text,
/// component_id) tuple — used by T5 to obtain the `row_id` that
/// `record_disambiguation_choice` requires.
async fn fetch_intent_row_id(
    pool: &deadpool_postgres::Pool,
    scope: &IntentScope,
    input_text: &str,
    component_id: Uuid,
) -> Uuid {
    let client = pool.get().await.expect("pool client");
    client
        .query_one(
            "SELECT id FROM reborn_intent_inputs
             WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3
               AND project_id = $4 AND input_text = $5 AND component_id = $6",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &input_text,
                &component_id,
            ],
        )
        .await
        .expect("fetch intent row id")
        .get(0)
}

// ---------------------------------------------------------------------------
// T1 — Recipe intent WITH step_link → step_link: Some, component_name: ""
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t1_recipe_intent_with_step_link_returns_some_and_empty_name() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let query = unique_sentence_query();
    let component_id = Uuid::new_v4();

    seed_intent_input(
        &rig.pool,
        &scope,
        &query,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        Some("1:1-1:3"),
    )
    .await
    .expect("seed intent");

    match resolve_intent(&rig.pool, &scope, &query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            step_link,
            ..
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert_eq!(step_link.as_deref(), Some("1:1-1:3"));
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T2 — non-Recipe intent WITHOUT step_link → step_link: None (legacy path)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t2_intent_without_step_link_returns_none_and_empty_name() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let query = unique_sentence_query();
    let component_id = Uuid::new_v4();

    // class 13 (tool_skill) — a non-Action, non-Recipe component seeded with
    // no step_link, exercising the legacy `fetch_component_by_id` contract.
    seed_intent_input(
        &rig.pool,
        &scope,
        &query,
        InputClass::Sentence,
        component_id,
        13,
        IntentSource::Seeded,
        None,
    )
    .await
    .expect("seed intent");

    match resolve_intent(&rig.pool, &scope, &query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            step_link,
            ..
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 13);
            assert_eq!(step_link, None);
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T3 — class-16 Action intent → class_code=16, step_link=None (HI.1: no JOIN)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t3_class16_action_intent_resolves_component_id() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let query = unique_sentence_query();

    // Insert the Action row (HI.1: no LEFT JOIN on reborn_actions in
    // resolve_intent — component_name removed from IntentResolution::Match).
    let action_id = insert_action_row(&rig.pool, &scope, "daily-sync").await;

    seed_intent_input(
        &rig.pool,
        &scope,
        &query,
        InputClass::Sentence,
        action_id,
        16,
        IntentSource::Seeded,
        None,
    )
    .await
    .expect("seed intent");

    match resolve_intent(&rig.pool, &scope, &query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            step_link,
            ..
        }) => {
            assert_eq!(cid, action_id);
            assert_eq!(component_class_code, 16);
            assert_eq!(step_link, None);
            // component_name removed from IntentResolution::Match (HI.1).
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T4 — class-21 Recipe intent (no Action row)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t4_class21_recipe_intent_without_action_row_has_empty_name() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let query = unique_sentence_query();
    let component_id = Uuid::new_v4();

    // No reborn_actions row for this component_id (class 21, not 16) → the
    // LEFT JOIN yields NULL → COALESCE(a.name, '') = ''.
    seed_intent_input(
        &rig.pool,
        &scope,
        &query,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        None,
    )
    .await
    .expect("seed intent");

    match resolve_intent(&rig.pool, &scope, &query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            step_link,
            ..
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert_eq!(step_link, None);
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T5 — choice retains the actual template and selected step link
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t5_disambiguation_choice_retains_actual_template_and_workflow() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let query = "reply with %".to_owned();
    let component_id = Uuid::new_v4();

    // A choice keeps the actual selected template and workflow for IBS.
    seed_intent_input(
        &rig.pool,
        &scope,
        &query,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        Some("1:1-1:3"),
    )
    .await
    .expect("seed intent");

    let row_id = fetch_intent_row_id(&rig.pool, &scope, &query, component_id).await;

    match record_disambiguation_choice(&rig.pool, &scope, row_id, component_id, 21).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            step_link,
            input_text,
            is_template,
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert_eq!(step_link.as_deref(), Some("1:1-1:3"));
            assert_eq!(input_text, query);
            assert!(is_template);
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[tokio::test]
async fn disambiguation_rejects_wrong_scope_identity_and_absent_rows_without_scoring() {
    use brassclaw_engine::memory::intent_system::IntentSystemError;
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let component = Uuid::new_v4();
    seed_intent_input(
        &rig.pool,
        &scope,
        "selected",
        InputClass::Word,
        component,
        21,
        IntentSource::Seeded,
        Some("0:1-0:E"),
    )
    .await
    .unwrap();
    let row = fetch_intent_row_id(&rig.pool, &scope, "selected", component).await;
    let mut bad_scopes = Vec::new();
    for field in 0..4 {
        let mut wrong = scope.clone();
        match field {
            0 => wrong.tenant_id.push_str("-wrong"),
            1 => wrong.user_id.push_str("-wrong"),
            2 => wrong.agent_id.push_str("-wrong"),
            _ => wrong.project_id.push_str("-wrong"),
        }
        bad_scopes.push(wrong);
    }
    for wrong in bad_scopes {
        assert!(matches!(
            record_disambiguation_choice(&rig.pool, &wrong, row, component, 21).await,
            Err(IntentSystemError::InvalidChoice)
        ));
    }
    for (id, target, class) in [
        (row, Uuid::new_v4(), 21),
        (row, component, 22),
        (Uuid::new_v4(), component, 21),
    ] {
        assert!(matches!(
            record_disambiguation_choice(&rig.pool, &scope, id, target, class).await,
            Err(IntentSystemError::InvalidChoice)
        ));
    }
    assert_eq!(score(&rig.pool, row).await, 1);
    record_disambiguation_choice(&rig.pool, &scope, row, component, 21)
        .await
        .unwrap();
    assert_eq!(score(&rig.pool, row).await, 2);
}

async fn score(pool: &brassclaw_pg::PgPool, row: Uuid) -> i32 {
    pool.get()
        .await
        .unwrap()
        .query_one(
            "SELECT score FROM reborn_intent_inputs WHERE id=$1",
            &[&row],
        )
        .await
        .unwrap()
        .get(0)
}

#[tokio::test]
async fn score_buckets_use_distinct_tuple_keys_and_preserve_exhausted_choices() {
    let rig = native_pg::NativePostgres::start().await;
    let unique = Uuid::new_v4().to_string();
    let first = IntentScope {
        tenant_id: format!("{unique}/b"),
        user_id: "c".into(),
        agent_id: "a".into(),
        project_id: "p".into(),
    };
    let second = IntentScope {
        tenant_id: unique,
        user_id: "b/c".into(),
        agent_id: "a".into(),
        project_id: "p".into(),
    };
    let mut rows = Vec::new();
    for scope in [&first, &second] {
        let component = Uuid::new_v4();
        seed_intent_input(
            &rig.pool,
            scope,
            "selected",
            InputClass::Word,
            component,
            21,
            IntentSource::Seeded,
            Some("0:1-0:E"),
        )
        .await
        .unwrap();
        rows.push((
            fetch_intent_row_id(&rig.pool, scope, "selected", component).await,
            component,
        ));
    }
    for _ in 0..51 {
        let result = record_disambiguation_choice(&rig.pool, &first, rows[0].0, rows[0].1, 21)
            .await
            .unwrap();
        assert!(
            matches!(result, IntentResolution::Match { step_link: Some(ref link), .. }
            if link == "0:1-0:E")
        );
    }
    assert_eq!(score(&rig.pool, rows[0].0).await, 51);
    record_disambiguation_choice(&rig.pool, &second, rows[1].0, rows[1].1, 21)
        .await
        .unwrap();
    assert_eq!(score(&rig.pool, rows[1].0).await, 2);
}

#[tokio::test]
async fn duplicate_templates_and_score_gaps_do_not_hide_other_workflows() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let recipe = Uuid::new_v4();
    let low = Uuid::new_v4();
    let query = format!("route{}end", "a".repeat(80));
    // More than the previous raw LIMIT 30, all for one workflow.
    for n in 0..40 {
        let template = format!("route{}%end", "a".repeat(n));
        seed_intent_input(
            &rig.pool,
            &scope,
            &template,
            InputClass::Word,
            recipe,
            21,
            IntentSource::Seeded,
            Some("0:1-0:E"),
        )
        .await
        .unwrap();
    }
    seed_intent_input(
        &rig.pool,
        &scope,
        "route%end",
        InputClass::Partial,
        low,
        21,
        IntentSource::Seeded,
        Some("2:1-2:E"),
    )
    .await
    .unwrap();
    seed_intent_input(
        &rig.pool,
        &scope,
        "route%end",
        InputClass::Sentence,
        recipe,
        21,
        IntentSource::Seeded,
        Some("1:1-1:E"),
    )
    .await
    .unwrap();
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE reborn_intent_inputs SET score = CASE WHEN component_id=$1 THEN 7 ELSE 10 END
        WHERE tenant_id=$2",
            &[&low, &scope.tenant_id],
        )
        .await
        .unwrap();
    let result = resolve_intent(&rig.pool, &scope, &query).await.unwrap();
    let IntentResolution::Disambiguation { candidates } = result else {
        panic!("distinct workflows must remain ambiguous: {result:?}");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|c| c.component_id == recipe));
    assert_eq!(candidates[0].step_link.as_deref(), Some("0:1-0:E"));
    assert_eq!(candidates[1].step_link.as_deref(), Some("1:1-1:E"));
    assert_ne!(candidates[0].row_id, candidates[1].row_id);
}

#[tokio::test]
async fn template_anchors_treat_underscore_backslash_and_escape_char_as_literals() {
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let recipe = Uuid::new_v4();
    for (template, valid, invalid) in [
        (r"read_a\%", r"read_a\value", r"readXa\value"),
        (r"%_done!\", r"value_done!\", r"valueXdone!\"),
        ("read!%_done", "read!value_done", "read!valueXdone"),
    ] {
        seed_intent_input(
            &rig.pool,
            &scope,
            template,
            InputClass::Word,
            recipe,
            21,
            IntentSource::Seeded,
            Some("0:1-0:E"),
        )
        .await
        .unwrap();
        assert!(matches!(
            resolve_intent(&rig.pool, &scope, invalid).await.unwrap(),
            IntentResolution::NoMatch
        ));
        assert!(matches!(
            resolve_intent(&rig.pool, &scope, valid).await.unwrap(),
            IntentResolution::Match { .. }
        ));
    }
}

#[tokio::test]
async fn caller_owned_snapshot_keeps_matching_stable_across_actual_reseeding() {
    use brassclaw_engine::memory::intent_system::{
        IntentSystemError, resolve_intent_in_transaction,
    };
    let rig = native_pg::NativePostgres::start().await;
    let scope = unique_scope();
    let recipe = Uuid::new_v4();
    seed_intent_input(
        &rig.pool,
        &scope,
        "selected",
        InputClass::Word,
        recipe,
        21,
        IntentSource::Seeded,
        Some("0:1-0:E"),
    )
    .await
    .unwrap();
    let row = fetch_intent_row_id(&rig.pool, &scope, "selected", recipe).await;
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let before = resolve_intent_in_transaction(&tx, &scope, "selected")
        .await
        .unwrap();
    assert!(
        matches!(before, IntentResolution::Match { step_link: Some(ref link), .. }
        if link == "0:1-0:E")
    );
    // A different real connection commits replacement intent metadata while
    // this caller still holds its matching/approval/IBS database view.
    seed_intent_input(
        &rig.pool,
        &scope,
        "selected",
        InputClass::Word,
        recipe,
        21,
        IntentSource::Seeded,
        Some("1:1-1:E"),
    )
    .await
    .unwrap();
    let still = resolve_intent_in_transaction(&tx, &scope, "selected")
        .await
        .unwrap();
    assert!(
        matches!(still, IntentResolution::Match { step_link: Some(ref link), .. }
        if link == "0:1-0:E")
    );
    assert!(matches!(
        resolve_intent_in_transaction(&tx, &scope, "absent")
            .await
            .unwrap(),
        IntentResolution::NoMatch
    ));
    tx.commit().await.unwrap();
    assert_eq!(
        score(&rig.pool, row).await,
        1,
        "snapshot preparation does not write telemetry"
    );
    let tx = client.transaction().await.unwrap();
    assert!(matches!(
        resolve_intent_in_transaction(&tx, &scope, "selected").await,
        Err(IntentSystemError::SnapshotIsolation)
    ));
    tx.rollback().await.unwrap();
    let after = resolve_intent(&rig.pool, &scope, "selected").await.unwrap();
    assert!(
        matches!(after, IntentResolution::Match { step_link: Some(ref link), .. }
        if link == "1:1-1:E")
    );
    assert_eq!(
        score(&rig.pool, row).await,
        2,
        "ordinary resolution retains scoring"
    );
    drop(client);
    rig.pool.close();
    assert!(
        matches!(
            resolve_intent(&rig.pool, &scope, "absent").await,
            Err(IntentSystemError::Db(_))
        ),
        "lookup failure cannot authorize Tier 2 as No-Match"
    );
}
