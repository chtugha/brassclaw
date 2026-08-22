//!
//! Integration tests for the Phase F `__fetch_component__` path and the
//! cross-tenant isolation guarantee (the core F.1–F.4 security fix).
//!
//! `__fetch_component__(uuid, class_code)` (registered F.6) delegates to
//! [`fetch_component_by_id`]; these tests drive that live engine API — the same
//! approach `fetch_for_turn.rs` takes with `PostgresSource::fetch_for_turn`
//! (the dormant Python-VM handler is thin glue over it, and `brassclaw_engine`
//! has no testcontainers dev-dep, so DB-backed tests live here at the
//! composition `tests/` tier). Each test starts an isolated Postgres-16
//! testcontainer, runs the full migration set (V000–V061), seeds, calls, and
//! asserts. Returns early (pass) when docker/testcontainers is unavailable.
//! Gated to `skills-db` because `fetch_component_by_id` / `PostgresSource` are
//! skills-db-only.
//!
//! Plan Phase F DB-integration test list → test fn map:
//! - #8 `__fetch_component__(uuid, 16)` → correct Action item
//!   → `fetch_component_by_id_returns_action_item`
//! - #9 two-tenant isolation (A's intents do NOT match for B's thread)
//!   → `cross_tenant_intent_isolation`
//!
#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::fetch_component_by_id;
use brassclaw_engine::memory::{
    ComponentScope, FetchForTurnResult, PostgresSource, RetrievalSource,
};
use tokio_postgres::types::ToSql;
use uuid::Uuid;

const SENDER: &str = "02:orchestrator";
const TOKEN_BUDGET: usize = 8000;

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: deadpool_postgres::Pool,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration (V000–V061). Returns `None` (skip) when docker is unavailable.
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
                "skipping fetch_component tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping fetch_component tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping fetch_component tests: no port ({error})");
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
/// collide on any component / intent row.
fn unique_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: format!("t-{}", Uuid::new_v4()),
        user_id: "u".to_string(),
        agent_id: "a".to_string(),
        project_id: "p".to_string(),
    }
}

/// A slug-style name satisfying the `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$` (1-64)
/// CHECK on `reborn_actions`. UUID-derived so parallel runs stay off the
/// `UNIQUE(scope, name)` constraint.
fn unique_name(prefix: &str) -> String {
    format!("{}{}", prefix, &Uuid::new_v4().simple().to_string()[..12])
}

/// Build a `PostgresSource` over the rig's pool (cheap pool clone → Arc).
fn source(rig: &PgRig) -> PostgresSource {
    PostgresSource::new(Arc::new(rig.pool.clone()))
}

// ---------------------------------------------------------------------------
// Seed helpers — supply only NOT-NULL-no-default columns; every other column
// has a default. The V061 `maintain_components_registry` AFTER-INSERT trigger
// auto-populates `reborn_components` on every insert.
// ---------------------------------------------------------------------------

async fn insert_action(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    name: &str,
) {
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_actions
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"action description",
                &"validated",
            ],
        )
        .await
        .expect("insert reborn_actions");
}

#[allow(clippy::too_many_arguments)]
async fn insert_intent_input(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    input_text: &str,
    input_class: i16,
    component_id: Uuid,
    component_class_code: i32,
    score: i32,
    step_link: Option<String>,
) {
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_intent_inputs
                 (tenant_id, user_id, agent_id, project_id, input_text, input_class,
                  component_id, component_class_code, score, step_link)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &scope.tenant_id as &(dyn ToSql + Sync),
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &input_text,
                &input_class,
                &component_id,
                &component_class_code,
                &score,
                &step_link,
            ],
        )
        .await
        .expect("insert reborn_intent_inputs");
}

// ---------------------------------------------------------------------------
// #8 — `__fetch_component__(uuid, 16)` returns the correct Action item.
// Drives `fetch_component_by_id` (the live engine API the F.6 handler calls).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_component_by_id_returns_action_item() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    insert_action(&rig.pool, &scope, action_id, &action_name).await;

    // `__fetch_component__(uuid, 16)` delegates to fetch_component_by_id.
    let items = fetch_component_by_id(&rig.pool, &scope, action_id, 16)
        .await
        .expect("fetch_component_by_id succeeds");

    assert_eq!(items.len(), 1, "exactly one validated action for the uuid");
    let item = &items[0];
    assert_eq!(item.id, action_id, "correct id");
    assert_eq!(item.class_code, 16, "correct class_code");
    assert_eq!(item.name, action_name, "correct name");
    // content_expr for class 16 = COALESCE(prior_knowledge_content, description);
    // the seed sets description = "action description" and no prior_knowledge_content.
    assert_eq!(item.effective_content, "action description");
    // Actions default to Solution Override (V029 schema: override_prompt_creation
    // DEFAULT true). The SEC-01 gate still returns the row because consumer_tags
    // defaults to '{}' and '05:validator' != ALL('{}') is vacuously true.
    assert!(
        item.override_prompt_creation,
        "actions default to override_prompt_creation = true"
    );
}

// ---------------------------------------------------------------------------
// #9 — Cross-tenant isolation (the core F.1–F.4 security fix). Tenant A's
// intents must NOT match for tenant B's thread: `resolve_intent` /
// `fetch_for_turn` filter on `tenant_id = scope.tenant_id`, so an empty tenant
// B issuing A's exact query gets no match (and never A's action id).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_tenant_intent_isolation() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope_a = unique_scope();
    let scope_b = unique_scope();

    // Tenant A owns an Action + its intent input.
    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    insert_action(&rig.pool, &scope_a, action_id, &action_name).await;
    insert_intent_input(&rig.pool, &scope_a, "run job", 2, action_id, 16, 10, None).await;

    // Positive control: tenant A's query resolves to A's action.
    let result_a = source(&rig)
        .fetch_for_turn(&scope_a, "run job", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");
    match result_a {
        FetchForTurnResult::ActionShortCircuit { component_id, name } => {
            assert_eq!(component_id, action_id, "tenant A matches its own action");
            assert_eq!(name, action_name);
        }
        other => panic!("tenant A expected ActionShortCircuit, got {other:?}"),
    }

    // Negative: tenant B issues the SAME query — must NOT resolve to A's action.
    let result_b = source(&rig)
        .fetch_for_turn(&scope_b, "run job", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");
    match result_b {
        FetchForTurnResult::ActionShortCircuit { component_id, .. } => {
            panic!("CROSS-TENANT LEAK: tenant B resolved tenant A's action {component_id}");
        }
        FetchForTurnResult::Components(items) => {
            // Tenant B has no components — the broad scan returns nothing,
            // and crucially nothing matching A's action id.
            assert!(
                !items.iter().any(|i| i.id == action_id),
                "tenant B must not receive tenant A's action component"
            );
        }
        other => panic!("tenant B expected Components (no match), got {other:?}"),
    }
}
