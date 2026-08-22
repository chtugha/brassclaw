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
//! testcontainer, runs the full migration set (V000–V062), seeds, calls, and
//! asserts. Returns early (pass) when docker/testcontainers is unavailable.
//! Gated to `skills-db` because `fetch_component_by_id` / `PostgresSource` are
//! skills-db-only.
//!
//! Plan Phase F / G DB-integration test list → test fn map:
//! - #8 `__fetch_component__(uuid, 16)` → correct Action item
//!   → `fetch_component_by_id_returns_action_item`
//! - #9 two-tenant isolation (A's intents do NOT match for B's thread)
//!   → `cross_tenant_intent_isolation`
//! - Phase G.2 `__resolve_component_by_name__(name, 16)` → correct Action item and tenant scoping → `fetch_component_by_name_resolves_action_item`, `fetch_component_by_name_is_tenant_scoped`
//! - Phase G / Q-G-STUB1 class-16 fetch surfaces executable `steps` and `allowed_tools` by id → `fetch_component_by_id_returns_action_steps`
//! - Phase G / Q-G-STUB1 class-16 fetch surfaces executable `steps` and `allowed_tools` by name → `fetch_component_by_name_returns_action_steps`
//!
#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::{fetch_component_by_id, fetch_component_by_name};
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
// Phase G.2 — `__resolve_component_by_name__(name, class_code)` resolves the
// correct validated Action item by name. Drives `fetch_component_by_name`
// (the live engine API the G.2 handler calls), mirroring #8 above. The
// SEC-01 gate is exercised by the cross-tenant case below: a name owned by
// tenant A must NOT resolve for tenant B.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_component_by_name_resolves_action_item() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    insert_action(&rig.pool, &scope, action_id, &action_name).await;

    // `__resolve_component_by_name__(name, 16)` delegates to
    // fetch_component_by_name — must return the same item #8 found by id.
    let items = fetch_component_by_name(&rig.pool, &scope, &action_name, 16)
        .await
        .expect("fetch_component_by_name succeeds");

    assert_eq!(items.len(), 1, "exactly one validated action for the name");
    let item = &items[0];
    assert_eq!(item.id, action_id, "correct id");
    assert_eq!(item.class_code, 16, "correct class_code");
    assert_eq!(item.name, action_name, "correct name");
    assert_eq!(item.effective_content, "action description");
    assert!(
        item.override_prompt_creation,
        "actions default to override_prompt_creation = true"
    );
}

#[tokio::test]
async fn fetch_component_by_name_is_tenant_scoped() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope_a = unique_scope();
    let scope_b = unique_scope();

    // Tenant A owns a validated Action with a unique name.
    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    insert_action(&rig.pool, &scope_a, action_id, &action_name).await;

    // Positive control: tenant A resolves A's action by name.
    let items_a = fetch_component_by_name(&rig.pool, &scope_a, &action_name, 16)
        .await
        .expect("fetch_component_by_name succeeds");
    assert_eq!(items_a.len(), 1, "tenant A resolves its own action");
    assert_eq!(items_a[0].id, action_id);

    // Negative: tenant B issues the SAME name — must NOT resolve (empty vec,
    // never A's action id). The `tenant_id = $2` scope filter prevents the
    // cross-tenant leak (the core F.1–F.4 / G.2 security guarantee).
    let items_b = fetch_component_by_name(&rig.pool, &scope_b, &action_name, 16)
        .await
        .expect("fetch_component_by_name succeeds");
    assert!(
        items_b.is_empty(),
        "CROSS-TENANT LEAK: tenant B resolved tenant A's action by name: {items_b:?}"
    );
}

// ---------------------------------------------------------------------------
// Phase G / Q-G-STUB1 — the class-16 fetch surfaces the executable `steps`
// (JSONB) + `allowed_tools` (TEXT[]) so `execute_action_procedure` can run the
// real procedure. Before the fix `__fetch_component__`/`__resolve_component_
// by_name__` returned a component *view* (description only) with no `steps`
// key, so `_execute_action_steps` saw `steps = []` and silently ran zero steps.
// These drive the live engine APIs the G.6 step-0 + `call_action` paths call.
// ---------------------------------------------------------------------------

/// Seed a validated Action carrying an explicit executable procedure (the
/// `steps` JSONB + `allowed_tools` TEXT[]). Mirrors `insert_action` but
/// populates the two columns the Q-G-STUB1 fetch branch projects.
async fn insert_action_with_procedure(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    name: &str,
    steps: &serde_json::Value,
    allowed_tools: &Vec<String>,
) {
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_actions
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  validation_status, steps, allowed_tools)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"action description",
                &"validated",
                steps,
                allowed_tools,
            ],
        )
        .await
        .expect("insert reborn_actions with procedure");
}

/// The known procedure the tests seed + assert against (kept shared so the
/// by-id and by-name variants assert the identical round-tripped shape).
fn known_procedure() -> (serde_json::Value, Vec<String>) {
    let steps = serde_json::json!([
        { "type": "tool_call", "tool": "shell", "args": { "cmd": "echo hi" } },
        { "type": "return", "value": "done" }
    ]);
    let allowed_tools = vec!["shell".to_string(), "memory".to_string()];
    (steps, allowed_tools)
}

#[tokio::test]
async fn fetch_component_by_id_returns_action_steps() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    let (steps, allowed_tools) = known_procedure();
    insert_action_with_procedure(
        &rig.pool,
        &scope,
        action_id,
        &action_name,
        &steps,
        &allowed_tools,
    )
    .await;

    // `__fetch_component__(uuid, 16)` delegates to fetch_component_by_id — the
    // Q-G-STUB1 class-16 branch must surface the executable steps + tools.
    let items = fetch_component_by_id(&rig.pool, &scope, action_id, 16)
        .await
        .expect("fetch_component_by_id succeeds");

    assert_eq!(items.len(), 1, "exactly one validated action for the uuid");
    let item = &items[0];
    assert_eq!(item.id, action_id, "correct id");
    assert_eq!(item.class_code, 16, "correct class_code");

    // The executable `steps` JSONB is surfaced for class 16 (Q-G-STUB1).
    let steps_val = item
        .steps
        .as_ref()
        .expect("class-16 fetch must surface executable steps");
    let steps_arr = steps_val.as_array().expect("steps must be a JSON array");
    assert_eq!(
        steps_arr.len(),
        2,
        "both authored steps survived the round-trip"
    );
    assert_eq!(
        steps_arr[0].get("type").and_then(|v| v.as_str()),
        Some("tool_call")
    );
    assert_eq!(
        steps_arr[1].get("type").and_then(|v| v.as_str()),
        Some("return")
    );

    // `allowed_tools` TEXT[] → JSON array of strings.
    let allowed_val = item
        .allowed_tools
        .as_ref()
        .expect("class-16 fetch must surface allowed_tools");
    let allowed_arr = allowed_val
        .as_array()
        .expect("allowed_tools must be a JSON array");
    assert_eq!(allowed_arr.len(), 2);
    assert!(allowed_arr.iter().any(|v| v.as_str() == Some("shell")));
    assert!(allowed_arr.iter().any(|v| v.as_str() == Some("memory")));
}

#[tokio::test]
async fn fetch_component_by_name_returns_action_steps() {
    // Symmetric to the by-id case: `__resolve_component_by_name__(name, 16)`
    // (the §0.9 Option B fallback) must ALSO surface the executable steps, so
    // a `call_action` that holds a step name (not a UUID) runs real steps too.
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    let (steps, allowed_tools) = known_procedure();
    insert_action_with_procedure(
        &rig.pool,
        &scope,
        action_id,
        &action_name,
        &steps,
        &allowed_tools,
    )
    .await;

    let items = fetch_component_by_name(&rig.pool, &scope, &action_name, 16)
        .await
        .expect("fetch_component_by_name succeeds");

    assert_eq!(items.len(), 1, "exactly one validated action for the name");
    let item = &items[0];
    assert_eq!(item.id, action_id, "correct id");

    let steps_val = item
        .steps
        .as_ref()
        .expect("by-name class-16 fetch must surface executable steps");
    assert_eq!(
        steps_val.as_array().map(Vec::len),
        Some(2),
        "both authored steps survived the round-trip"
    );
    assert!(item.allowed_tools.is_some(), "allowed_tools surfaced");
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
