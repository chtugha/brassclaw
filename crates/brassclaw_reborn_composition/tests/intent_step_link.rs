//! Phase D — `step_link` column on `reborn_intent_inputs` (§0.6 / §0.8).
//!
//! Integration tests for `resolve_intent` + `seed_intent_input` +
//! `record_disambiguation_choice` against a real Postgres-16 schema with the
//! V054 `step_link` column. The `engine` crate has no testcontainers dev-dep,
//! so these DB-backed intent tests live in the composition `tests/` tier
//! (established Phase B/C pattern) and mirror `extension_catalogue_component`.
//!
//! Verifies the five Phase D behaviours (plan §'Phase D — Tests'):
//! - T1: class-21 Recipe intent seeded WITH `step_link` →
//!   `Match { step_link: Some("1:1-1:3"), component_name: "" }`
//! - T2: non-Recipe intent seeded WITHOUT `step_link` →
//!   `Match { step_link: None, component_name: "" }` (legacy path unchanged)
//! - T3: class-16 Action intent →
//!   `Match { component_class_code: 16, component_name: "daily-sync" }`
//!   — name populated via the scope-filtered LEFT JOIN on `reborn_actions`
//!   (FIND-P6-05 security requirement)
//! - T4: class-21 Recipe intent (no Action row) →
//!   `Match { component_class_code: 21, component_name: "" }` — empty name
//!   for non-Action matches (COALESCE over a NULL JOIN row)
//! - T5: `record_disambiguation_choice` →
//!   `Match { step_link: None, component_name: "" }` (FINDING A)
//!
//! Each test starts an isolated Postgres-16 testcontainer, runs the full
//! migration set (V000–V054, so `step_link` exists), and returns early (pass)
//! when docker/testcontainers is unavailable. Gated to `skills-db` because
//! `resolve_intent` / `seed_intent_input` / `record_disambiguation_choice` are
//! skills-db-only.

#![cfg(feature = "skills-db")]

use brassclaw_engine::memory::intent_system::{
    InputClass, IntentResolution, IntentScope, IntentSource, record_disambiguation_choice,
    resolve_intent, seed_intent_input,
};
use uuid::Uuid;

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: deadpool_postgres::Pool,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration (V000–V054, so `reborn_intent_inputs.step_link` exists). Returns
/// `None` (skip) when docker is unavailable.
async fn pg_rig_or_skip() -> Option<PgRig> {
    use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

    let image = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("brassclaw_test")
        .with_user("postgres")
        .with_password("postgres")
        .with_tag("16-alpine");
    let container = match image.start().await {
        Ok(c) => c,
        Err(error) => {
            eprintln!(
                "skipping intent_step_link tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping intent_step_link tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping intent_step_link tests: no port ({error})");
            return None;
        }
    };
    let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
    let cfg: tokio_postgres::Config = url.parse().expect("testcontainer url parses");
    let manager = deadpool_postgres::Manager::new(cfg, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .expect("Postgres pool must build");
    brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .expect("migrations must apply");
    Some(PgRig {
        _container: container,
        pool,
    })
}

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
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
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
            component_name,
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert_eq!(step_link.as_deref(), Some("1:1-1:3"));
            assert_eq!(component_name, "");
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T2 — non-Recipe intent WITHOUT step_link → step_link: None (legacy path)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t2_intent_without_step_link_returns_none_and_empty_name() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
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
            component_name,
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 13);
            assert_eq!(step_link, None);
            assert_eq!(component_name, "");
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T3 — class-16 Action intent → name populated from the scope-filtered JOIN
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t3_class16_action_intent_populates_component_name_via_join() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let query = unique_sentence_query();

    // Insert the Action row in the SAME scope so the LEFT JOIN (FIND-P6-05:
    // all 4 scope filters) resolves component_name to the Action's name.
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
            component_name,
        }) => {
            assert_eq!(cid, action_id);
            assert_eq!(component_class_code, 16);
            assert_eq!(step_link, None);
            assert_eq!(component_name, "daily-sync");
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T4 — class-21 Recipe intent (no Action row) → empty component_name
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t4_class21_recipe_intent_without_action_row_has_empty_name() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
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
            component_name,
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert_eq!(step_link, None);
            assert_eq!(component_name, "");
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T5 — record_disambiguation_choice → step_link: None, component_name: ""
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t5_record_disambiguation_choice_returns_none_step_link_and_empty_name() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let query = unique_sentence_query();
    let component_id = Uuid::new_v4();

    // Seed a Recipe intent (carrying a step_link) so the row exists; the
    // disambiguation click confirms component_id only and must NOT echo the
    // stored step_link — FINDING A mandates step_link: None (caller re-fetches
    // the recipe row) and component_name: "" (disambiguation is Recipe/Skill,
    // never an Action).
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
            component_name,
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert_eq!(step_link, None);
            assert_eq!(component_name, "");
        }
        other => panic!("expected Match, got {other:?}"),
    }
}
