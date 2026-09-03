//!
//! Phase C.2 integration test — the `builtin-host` component seed
//! ([`brassclaw_reborn_composition::seed_builtin_host::seed_builtin_host_components`]).
//!
//! Starts an isolated Postgres-16 testcontainer, runs the full migration set,
//! calls the idempotent boot seed, and asserts the Step 27 component stacks
//! landed as `source = "system"` + `validation_status = "validated"` rows:
//! 8 `host.*` Tools, 8 `ts-host-*` ToolSkills, 12 `pc-host-*` PythonCodes,
//! 8 `skill-host-*` leaf Skills, 6 `host-*` Recipes, and the class-23
//! `builtin-host` ExtensionCatalogue row whose `child_component_ids` resolve.
//! Re-runs the seed to prove idempotency (counts unchanged). Returns early
//! (pass) when docker/testcontainers is unavailable. Gated to `skills-db`
//! (which implies `postgres`) because the seed + stores are postgres-only.
//!
//! This is the only automated regression check that the seed produces the
//! correct rows — the boot path in `webui.rs` swallows seed errors as a
//! `tracing::warn!`, so a regression would not fail the webui E2E suite.

#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use brassclaw_reborn_composition::seed_builtin_host::seed_builtin_host_components;
use tokio_postgres::types::ToSql;
use uuid::Uuid;

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: PgPool,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration. Returns `None` (skip) when docker is unavailable.
async fn pg_rig_or_skip() -> Option<PgRig> {
    use testcontainers_modules::testcontainers::{runners::AsyncRunner, ImageExt};

    let image = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("brassclaw_test")
        .with_user("postgres")
        .with_password("postgres")
        .with_tag("16-alpine");
    let container = match image.start().await {
        Ok(c) => c,
        Err(error) => {
            eprintln!(
                "skipping builtin_host_seed tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping builtin_host_seed tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping builtin_host_seed tests: no port ({error})");
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

/// Count rows in `table` matching `name LIKE $prefix%` for the seed's marker
/// scope `(tenant, SYSTEM_RESERVED_ID, "default", "system")`, all
/// `source = 'system'` + `validation_status = 'validated'`.
async fn count_validated_system(
    pool: &PgPool,
    table: &str,
    tenant: &str,
    prefix: &str,
) -> i64 {
    let client = pool.get().await.expect("pool client");
    let user_id = SYSTEM_RESERVED_ID.to_string();
    let prefix_like = format!("{prefix}%");
    let sql = format!(
        "SELECT COUNT(*) FROM {table}
         WHERE tenant_id = $1
           AND user_id = $2
           AND agent_id = 'default'
           AND project_id = 'system'
           AND source = 'system'
           AND validation_status = 'validated'
           AND name LIKE $3"
    );
    let params: &[&(dyn ToSql + Sync)] = &[&tenant, &user_id, &prefix_like];
    let row = client
        .query_one(&sql, params)
        .await
        .expect("count query");
    row.get(0)
}

/// Fetch the `builtin-host` catalogue row's `child_component_ids`.
async fn catalogue_children(pool: &PgPool, tenant: &str) -> Vec<Uuid> {
    let client = pool.get().await.expect("pool client");
    let user_id = SYSTEM_RESERVED_ID.to_string();
    let params: &[&(dyn ToSql + Sync)] = &[&tenant, &user_id];
    let row = client
        .query_one(
            "SELECT child_component_ids, validation_status, source
             FROM reborn_extension_catalogues
             WHERE tenant_id = $1
               AND user_id = $2
               AND agent_id = 'default'
               AND project_id = 'system'
               AND name = 'builtin-host'",
            params,
        )
        .await
        .expect("builtin-host catalogue row must exist");
    assert_eq!(
        row.get::<_, String>(1),
        "validated",
        "builtin-host catalogue must be validated"
    );
    assert_eq!(
        row.get::<_, String>(2),
        "system",
        "builtin-host catalogue must be source=system"
    );
    row.get::<_, Vec<Uuid>>(0)
}

/// Assert every id in `child_ids` resolves to an existing component row.
async fn assert_children_resolve(pool: &PgPool, child_ids: &[Uuid]) {
    let client = pool.get().await.expect("pool client");
    for id in child_ids {
        let exists: bool = client
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM reborn_components WHERE id = $1
                 )",
                &[id],
            )
            .await
            .expect("resolve child")
            .get(0);
        assert!(exists, "catalogue child {id} must resolve to a component");
    }
}

#[tokio::test]
async fn builtin_host_seed_lands_all_step27_components() {
    let Some(rig) = pg_rig_or_skip().await else {
        return;
    };
    let tenant = format!("t-{}", Uuid::new_v4());

    // First seed.
    seed_builtin_host_components(Arc::new(rig.pool.clone()), &tenant)
        .await
        .expect("seed must succeed");

    // 8 host.* Tools (class 0).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_tools", &tenant, "host.").await,
        8,
        "exactly 8 host.* tools"
    );
    // 8 ts-host-* ToolSkills (class 13).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant, "ts-host-").await,
        8,
        "exactly 8 ts-host-* tool skills"
    );
    // 12 pc-host-* PythonCodes (class 22): 8 tool handoffs + 4 recipe formatters.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_python_code", &tenant, "pc-host-").await,
        12,
        "exactly 12 pc-host-* python codes"
    );
    // 8 skill-host-* leaf Skills (class 1).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_skills", &tenant, "skill-host-").await,
        8,
        "exactly 8 skill-host-* leaf skills"
    );
    // 6 host-* Recipes (class 21): resolve_intent / compose-and-run-orchestrator /
    // post-reply / save-history / assemble-prior-knowledge / non-match-llm-answer.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_recipes", &tenant, "host-").await,
        6,
        "exactly 6 host-* recipes"
    );

    // Catalogue row + its children resolve.
    let children = catalogue_children(&rig.pool, &tenant).await;
    assert!(
        !children.is_empty(),
        "builtin-host child_component_ids must be non-empty"
    );
    assert_children_resolve(&rig.pool, &children).await;

    // Idempotency: a re-seed leaves every count unchanged.
    let before = (
        count_validated_system(&rig.pool, "reborn_tools", &tenant, "host.").await,
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant, "ts-host-").await,
        count_validated_system(&rig.pool, "reborn_python_code", &tenant, "pc-host-").await,
        count_validated_system(&rig.pool, "reborn_skills", &tenant, "skill-host-").await,
        count_validated_system(&rig.pool, "reborn_recipes", &tenant, "host-").await,
    );
    seed_builtin_host_components(Arc::new(rig.pool.clone()), &tenant)
        .await
        .expect("re-seed must succeed");
    let after = (
        count_validated_system(&rig.pool, "reborn_tools", &tenant, "host.").await,
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant, "ts-host-").await,
        count_validated_system(&rig.pool, "reborn_python_code", &tenant, "pc-host-").await,
        count_validated_system(&rig.pool, "reborn_skills", &tenant, "skill-host-").await,
        count_validated_system(&rig.pool, "reborn_recipes", &tenant, "host-").await,
    );
    assert_eq!(before, after, "re-seed must be idempotent");
}
