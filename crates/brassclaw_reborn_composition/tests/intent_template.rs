//! Phase M.3 — Variable Intent Templates: three-path `resolve_intent` SQL
//! (§0.17.1).
//!
//! Integration tests for the V076 template columns (`is_template` /
//! `template_prefix` / `template_suffix`) against a real Postgres-16 schema
//! with the full migration set (V000–V076). Mirrors the `intent_step_link`
//! rig pattern (established Phase B/C/D pattern: each DB-backed intent test
//! file owns its own `PgRig` + `pg_rig_or_skip`).
//!
//! Verifies the three §0.17.1 dispatch paths through `resolve_intent`:
//! - T1: prefix-anchored template `"show me all files in the % directory"`
//!   matches the concrete query `"show me all files in the /tmp directory"`
//!   via Path 1 (B-tree index on `template_prefix`) →
//!   `Match { is_template: true, input_text: <template> }`.
//! - T2: for the SAME component, an exact literal row AND a template row both
//!   match a query equal to the literal; the `CASE WHEN input_text = $5 THEN
//!   0 ELSE 1 END` ORDER BY tiebreaker ranks the exact row first, and the
//!   per-`component_id` dedup collapses them to one `Match` carrying the exact
//!   row's `is_template: false` + `input_text: <literal>` (§0.17.1 exact-beats-
//!   template invariant).
//! - T3: suffix-anchored (leading-`%`) template `"% directory"` matches the
//!   concrete query `"please list the contents of the /tmp directory"` via
//!   Path 2 (functional B-tree index on `reverse(template_suffix)`) →
//!   `Match { is_template: true, input_text: "% directory" }`.
//!
//! Each test starts an isolated Postgres-16 testcontainer, runs every
//! migration (V000–V076, so the template columns + partial indexes exist), and
//! returns early (pass) when docker/testcontainers is unavailable. Gated to
//! `skills-db` because `resolve_intent` / `seed_intent_input` are
//! skills-db-only.

#![cfg(feature = "skills-db")]

use brassclaw_engine::memory::intent_system::{
    InputClass, IntentResolution, IntentScope, IntentSource, resolve_intent, seed_intent_input,
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
/// migration (V000–V076, so `reborn_intent_inputs.is_template` +
/// `template_prefix` + `template_suffix` + the two partial indexes exist).
/// Returns `None` (skip) when docker is unavailable.
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
                "skipping intent_template tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping intent_template tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping intent_template tests: no port ({error})");
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
/// collide on `reborn_intent_inputs` rows or the per-scope score rate-limit
/// bucket (SEC-05).
fn unique_scope() -> IntentScope {
    IntentScope {
        tenant_id: format!("t-{}", Uuid::new_v4()),
        user_id: "u".to_string(),
        agent_id: "a".to_string(),
        project_id: "p".to_string(),
    }
}

// ---------------------------------------------------------------------------
// T1 — prefix-anchored template matches via Path 1
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t1_prefix_anchored_template_matches_path1() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let component_id = Uuid::new_v4();
    // Template expression with a non-empty prefix anchor.
    let template = "show me all files in the % directory";
    // Concrete user text that fits the template (the `%` captures "/tmp").
    let query = "show me all files in the /tmp directory";

    seed_intent_input(
        &rig.pool,
        &scope,
        template,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        Some("1:1-1:3"),
    )
    .await
    .expect("seed template intent");

    match resolve_intent(&rig.pool, &scope, query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            is_template,
            input_text,
            ..
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert!(is_template, "template match must surface is_template=true");
            assert_eq!(
                input_text, template,
                "input_text must be the stored template expression"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T2 — exact literal outranks template for the same component (§0.17.1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t2_exact_match_outranks_template_for_same_component() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    // Both intent rows point at the SAME component so the per-component_id
    // dedup collapses them to a single Match (§0.17.1 exact-beats-template).
    let component_id = Uuid::new_v4();
    let literal = "show me all files in the current directory";
    let template = "show me all files in the % directory";
    // Query equals the literal exactly → Path 0 fires for the literal row AND
    // Path 1 fires for the template row; the ORDER BY tiebreaker must rank the
    // exact row first.
    let query = literal;

    seed_intent_input(
        &rig.pool,
        &scope,
        template,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        Some("1:1-1:3"),
    )
    .await
    .expect("seed template intent");
    seed_intent_input(
        &rig.pool,
        &scope,
        literal,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        Some("1:1-1:3"),
    )
    .await
    .expect("seed exact intent");

    match resolve_intent(&rig.pool, &scope, query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            is_template,
            input_text,
            ..
        }) => {
            assert_eq!(cid, component_id);
            assert!(
                !is_template,
                "exact match must win, surfacing is_template=false"
            );
            assert_eq!(
                input_text, literal,
                "winning row must be the exact literal, not the template"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T3 — suffix-anchored (leading-%) template matches via Path 2 reverse index
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t3_suffix_anchored_template_matches_path2_reverse_index() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let component_id = Uuid::new_v4();
    // Leading-% template: prefix is empty, suffix is " directory".
    let template = "% directory";
    // Concrete user text ending with the suffix; the `%` captures everything
    // before " directory".
    let query = "please list the contents of the /tmp directory";

    seed_intent_input(
        &rig.pool,
        &scope,
        template,
        InputClass::Sentence,
        component_id,
        21,
        IntentSource::Seeded,
        Some("1:1-1:3"),
    )
    .await
    .expect("seed suffix-anchored template intent");

    match resolve_intent(&rig.pool, &scope, query).await {
        Ok(IntentResolution::Match {
            component_id: cid,
            component_class_code,
            is_template,
            input_text,
            ..
        }) => {
            assert_eq!(cid, component_id);
            assert_eq!(component_class_code, 21);
            assert!(is_template, "template match must surface is_template=true");
            assert_eq!(
                input_text, template,
                "input_text must be the stored suffix-anchored template"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}
