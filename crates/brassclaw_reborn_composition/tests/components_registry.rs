//! Phase E — `reborn_components` registry (V061, FIND-IBS-02 resolution).
//!
//! Integration tests for `lookup_component_class` against a real Postgres-16
//! schema with the V061 `reborn_components` registry + the
//! `maintain_components_registry` triggers on all 14 class tables. The `engine`
//! crate has no testcontainers dev-dep, so these DB-backed tests live in the
//! composition `tests/` tier (established Phase B/C/D pattern) and mirror
//! `intent_step_link`.
//!
//! Verifies the four E.1 behaviours:
//! - T1: insert a `reborn_skills` row (class 3) → the AFTER-INSERT trigger
//!   maintains the registry, and `lookup_component_class` returns `Some(3)`.
//! - T2: insert a `reborn_actions` row (class 16, a DIFFERENT table) →
//!   `lookup_component_class` returns `Some(16)` — proves the generic trigger
//!   reads `NEW.class_code` correctly across tables (not a hardcoded class).
//! - T3: SEC-01 tenant isolation — a row in scope A must NOT resolve under a
//!   foreign tenant's scope; `lookup_component_class` returns `None`.
//! - T4: an absent UUID returns `None` (soft-skip contract — a missing include
//!   is a soft authoring gap, not a hard error).
//!
//! Each test starts an isolated Postgres-16 testcontainer and runs the full
//! migration set (V000–V061, so `reborn_components` + its triggers exist), and
//! returns early (pass) when docker/testcontainers is unavailable. Gated to
//! `skills-db` because `lookup_component_class` is skills-db-only.

#![cfg(feature = "skills-db")]

use brassclaw_engine::memory::{ComponentScope, retrieval_source::lookup_component_class};
use uuid::Uuid;

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: deadpool_postgres::Pool,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration (V000–V061, so `reborn_components` + its triggers exist). Returns
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
                "skipping components_registry tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping components_registry tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping components_registry tests: no port ({error})");
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
/// collide on `reborn_components` rows.
fn unique_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: format!("t-{}", Uuid::new_v4()),
        user_id: "u".to_string(),
        agent_id: "a".to_string(),
        project_id: "p".to_string(),
    }
}

/// A slug-style name satisfying `reborn_skills.name` CHECK
/// (`^[a-z0-9]([a-z0-9-]*[a-z0-9])?$`, length 1-64). UUID-derived so parallel
/// runs stay off the `UNIQUE(scope, name)` constraint.
fn unique_name(prefix: &str) -> String {
    format!("{}{}", prefix, &Uuid::new_v4().simple().to_string()[..12])
}

/// Insert a minimal class-3 `reborn_skills` row: only the NOT-NULL-no-default
/// columns (scope tuple + `name` + `description`) plus an explicit `class_code`
/// are supplied; every other column has a default. Returns the assigned row id.
async fn insert_skill_row(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    class_code: i16,
) -> Uuid {
    let id = Uuid::new_v4();
    let name = unique_name("skill");
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_skills
                 (id, tenant_id, user_id, agent_id, project_id, name, description, class_code)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"test skill description",
                &class_code,
            ],
        )
        .await
        .expect("insert reborn_skills");
    id
}

/// Insert a minimal class-16 `reborn_actions` row: only the NOT-NULL-no-default
/// columns (scope tuple + `name` + `description`) plus an explicit `class_code`
/// are supplied. Returns the assigned row id.
async fn insert_action_row(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    class_code: i16,
) -> Uuid {
    let id = Uuid::new_v4();
    let name = unique_name("act");
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_actions
                 (id, tenant_id, user_id, agent_id, project_id, name, description, class_code)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"test action description",
                &class_code,
            ],
        )
        .await
        .expect("insert reborn_actions");
    id
}

// ---------------------------------------------------------------------------
// T1 — reborn_skills (class 3) insert → trigger maintains registry → Some(3)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t1_skill_insert_resolves_class_via_registry() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let skill_id = insert_skill_row(&rig.pool, &scope, 3).await;

    let resolved = lookup_component_class(&rig.pool, &scope, skill_id)
        .await
        .expect("lookup must succeed");
    assert_eq!(resolved, Some(3));
}

// ---------------------------------------------------------------------------
// T2 — reborn_actions (class 16, different table) → Some(16)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t2_action_insert_resolves_class_via_registry() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let action_id = insert_action_row(&rig.pool, &scope, 16).await;

    let resolved = lookup_component_class(&rig.pool, &scope, action_id)
        .await
        .expect("lookup must succeed");
    assert_eq!(resolved, Some(16));
}

// ---------------------------------------------------------------------------
// T3 — SEC-01: a row in scope A must NOT resolve under a foreign tenant
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t3_foreign_tenant_scope_returns_none() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope_a = unique_scope();
    let skill_id = insert_skill_row(&rig.pool, &scope_a, 3).await;

    // Same user/agent/project but a DIFFERENT tenant — the scoped WHERE clause
    // must return no row so a foreign-tenant UUID never resolves (SEC-01).
    let mut scope_b = scope_a.clone();
    scope_b.tenant_id = format!("foreign-{}", Uuid::new_v4());

    let resolved = lookup_component_class(&rig.pool, &scope_b, skill_id)
        .await
        .expect("lookup must succeed");
    assert_eq!(resolved, None);
}

// ---------------------------------------------------------------------------
// T4 — an absent UUID returns None (soft-skip contract)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t4_absent_uuid_returns_none() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();
    let absent = Uuid::new_v4();

    let resolved = lookup_component_class(&rig.pool, &scope, absent)
        .await
        .expect("lookup must succeed");
    assert_eq!(resolved, None);
}
