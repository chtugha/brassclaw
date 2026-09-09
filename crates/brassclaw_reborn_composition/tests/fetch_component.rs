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
//! - Phase G / Q-G-STUB1 class-16 fetch surfaces `steps` by id → `fetch_component_by_id_returns_action_item_with_description` (HI.1: `steps`/`allowed_tools` removed from ComponentItem; IBS pipeline handles them at compose time)
//! - Phase G.8 `call_action` nested resolution by UUID (`__fetch_component__(action_id, 16)`) → `call_action_resolves_nested_action_by_uuid`
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
// Phase G / HI.1 — class-16 fetch surfaces description + class_code correctly.
// Post-HI.1: `steps` and `allowed_tools` are no longer fields on `ComponentItem`
// (they are consumed at composition time by `compose_action_program` in the
// IBS pipeline). These tests verify the basic ComponentItem shape returned by
// the fetch path.
// ---------------------------------------------------------------------------

/// Seed a validated Action carrying an explicit executable procedure (the
/// `steps` JSONB + `allowed_tools` TEXT[]). The DB columns still exist;
/// ComponentItem simply no longer surfaces them (consumed at compose time).
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

/// The known procedure the tests seed (DB columns still written; not surfaced
/// on ComponentItem post-HI.1 — consumed at compose time by IBS pipeline).
fn known_procedure() -> (serde_json::Value, Vec<String>) {
    let steps = serde_json::json!([
        { "type": "tool_call", "tool": "shell", "args": { "cmd": "echo hi" } },
        { "type": "return", "value": "done" }
    ]);
    let allowed_tools = vec!["shell".to_string(), "memory".to_string()];
    (steps, allowed_tools)
}

#[tokio::test]
async fn fetch_component_by_id_with_procedure_returns_action_item() {
    // Post-HI.1: `steps`/`allowed_tools` are not in ComponentItem even when
    // the DB row was seeded with them. The fetch returns id + class_code only.
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

    let items = fetch_component_by_id(&rig.pool, &scope, action_id, 16)
        .await
        .expect("fetch_component_by_id succeeds");

    assert_eq!(items.len(), 1, "exactly one validated action for the uuid");
    let item = &items[0];
    assert_eq!(item.id, action_id, "correct id");
    assert_eq!(item.class_code, 16, "correct class_code");
    // steps/allowed_tools are consumed by compose_action_program at IBS time;
    // they are not fields on ComponentItem post-HI.1.
}

#[tokio::test]
async fn fetch_component_by_name_returns_action_item() {
    // Symmetric by-name variant: ComponentItem has id + class_code but no
    // steps/allowed_tools post-HI.1.
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
    assert_eq!(item.class_code, 16, "correct class_code");
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

    // Positive control: tenant A's query resolves to A's action (Components arm,
    // HI.1 — ActionShortCircuit was removed; class-16 routes through
    // fetch_component_by_id → Components).
    let result_a = source(&rig)
        .fetch_for_turn(&scope_a, "run job", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");
    match result_a {
        FetchForTurnResult::Components(items) => {
            assert!(
                items.iter().any(|i| i.id == action_id),
                "tenant A must resolve its own action; got: {items:?}"
            );
        }
        other => panic!("tenant A expected Components, got {other:?}"),
    }

    // Negative: tenant B issues the SAME query — must NOT resolve to A's action.
    let result_b = source(&rig)
        .fetch_for_turn(&scope_b, "run job", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");
    match result_b {
        FetchForTurnResult::Components(items) => {
            // Tenant B has no components — the broad scan returns nothing,
            // and crucially nothing matching A's action id.
            assert!(
                !items.iter().any(|i| i.id == action_id),
                "CROSS-TENANT LEAK: tenant B received tenant A's action component"
            );
        }
        other => panic!("tenant B expected Components (no match), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Phase G.8 — `call_action` nested resolution by UUID. A parent Action's
// `call_action` step holds a child `action_id` (UUID);
// `__fetch_component__(nested_action_id, 16)` → `fetch_component_by_id`.
// Post-HI.1: `steps`/`allowed_tools` are not on ComponentItem; both child and
// parent resolve by UUID and the ComponentItem's id + class_code are correct.
// The actual execution is handled by compose_action_program at IBS time.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn call_action_resolves_nested_action_by_uuid() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    // Child Action: a trivial executable procedure (a single return step).
    let child_id = Uuid::new_v4();
    let child_name = unique_name("child");
    let child_steps = serde_json::json!([{ "type": "return", "value": "child-done" }]);
    let child_tools = vec!["shell".to_string()];
    insert_action_with_procedure(
        &rig.pool,
        &scope,
        child_id,
        &child_name,
        &child_steps,
        &child_tools,
    )
    .await;

    // Parent Action: its procedure is a `call_action` step that references the
    // child by UUID (`action_id`).
    let parent_id = Uuid::new_v4();
    let parent_name = unique_name("parent");
    let parent_steps =
        serde_json::json!([{ "type": "call_action", "action": child_name, "action_id": child_id }]);
    let parent_tools = vec!["shell".to_string()];
    insert_action_with_procedure(
        &rig.pool,
        &scope,
        parent_id,
        &parent_name,
        &parent_steps,
        &parent_tools,
    )
    .await;

    // The exact resolution `call_action` performs: fetch the child by UUID.
    // Post-HI.1: steps/allowed_tools not on ComponentItem — only id/class_code checked.
    let child_items = fetch_component_by_id(&rig.pool, &scope, child_id, 16)
        .await
        .expect("fetch_component_by_id (child) succeeds");
    assert_eq!(child_items.len(), 1, "nested child action resolves by uuid");
    let child = &child_items[0];
    assert_eq!(child.id, child_id, "correct child id");
    assert_eq!(child.class_code, 16);

    // Parent resolves by UUID too.
    let parent_items = fetch_component_by_id(&rig.pool, &scope, parent_id, 16)
        .await
        .expect("fetch_component_by_id (parent) succeeds");
    assert_eq!(parent_items.len(), 1, "parent action resolves by uuid");
    let parent = &parent_items[0];
    assert_eq!(parent.id, parent_id, "correct parent id");
    assert_eq!(parent.class_code, 16);
}
