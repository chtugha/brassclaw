//!
//! Phase L integration test — the v3 builtin-tool component seed
//! ([`brassclaw_reborn_composition::builtin_bootstrap::seed_builtin_components`]).
//!
//! Starts an isolated Postgres-16 testcontainer, runs the full migration set,
//! calls the idempotent boot seed, and asserts the five domain groups
//! (filesystem → network → memory → process → management → host) landed as
//! `source = "system"` + `validation_status = "validated"` rows:
//! 23 Tools, 30 ToolSkills, 84 PythonCodes, 108 Skills (99 leaf + 9 domain),
//! 111 Recipes, and 24 ExtensionCatalogues (380 components total — the plan's
//! 319 target plus variants/helpers/gap-fillers identified during transcription,
//! plus the K4 host-assemble-prior-knowledge fallback recipe + its formatter).
//! Re-runs the seed to prove idempotency (counts unchanged). Also guards the
//! safety-critical content: `ts-spawn-subagent` carries "scope isolation" and
//! the `skill-shell-safe-check` skill body carries the "approval" rule.
//! Returns early (pass) when docker/testcontainers is unavailable. Gated to
//! `skills-db` (which implies `postgres`) because the seed + stores are
//! postgres-only.
//!
//! This is the only automated regression check that the seed produces the
//! correct rows — the boot path in `webui.rs` swallows seed errors as a
//! `tracing::warn!`, so a regression would not fail the webui E2E suite.

#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use brassclaw_reborn_composition::builtin_bootstrap::seed_builtin_components;
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
                "skipping builtin_bootstrap_seed tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping builtin_bootstrap_seed tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping builtin_bootstrap_seed tests: no port ({error})");
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

/// Count rows in `table` for the seed's marker scope
/// `(tenant, SYSTEM_RESERVED_ID, "default", "system")`, all
/// `source = 'system'` + `validation_status = 'validated'`.
async fn count_validated_system(pool: &PgPool, table: &str, tenant: &str) -> i64 {
    let client = pool.get().await.expect("pool client");
    let user_id = SYSTEM_RESERVED_ID.to_string();
    let sql = format!(
        "SELECT COUNT(*) FROM {table}
         WHERE tenant_id = $1
           AND user_id = $2
           AND agent_id = 'default'
           AND project_id = 'system'
           AND source = 'system'
           AND validation_status = 'validated'"
    );
    let params: &[&(dyn ToSql + Sync)] = &[&tenant, &user_id];
    let row = client
        .query_one(&sql, params)
        .await
        .expect("count query");
    row.get(0)
}

/// Fetch the `content` body of a ToolSkill row by name (seed marker scope).
async fn tool_skill_content(pool: &PgPool, tenant: &str, name: &str) -> String {
    let client = pool.get().await.expect("pool client");
    let user_id = SYSTEM_RESERVED_ID.to_string();
    let params: &[&(dyn ToSql + Sync)] = &[&tenant, &user_id, &name];
    let row = client
        .query_one(
            "SELECT content FROM reborn_tool_skills
             WHERE tenant_id = $1
               AND user_id = $2
               AND agent_id = 'default'
               AND project_id = 'system'
               AND source = 'system'
               AND name = $3",
            params,
        )
        .await
        .expect("toolskill row must exist");
    row.get::<_, String>(0)
}

/// Fetch the `body` of a Skill row by name (seed marker scope).
async fn skill_body(pool: &PgPool, tenant: &str, name: &str) -> String {
    let client = pool.get().await.expect("pool client");
    let user_id = SYSTEM_RESERVED_ID.to_string();
    let params: &[&(dyn ToSql + Sync)] = &[&tenant, &user_id, &name];
    let row = client
        .query_one(
            "SELECT body FROM reborn_skills
             WHERE tenant_id = $1
               AND user_id = $2
               AND agent_id = 'default'
               AND project_id = 'system'
               AND source = 'system'
               AND name = $3",
            params,
        )
        .await
        .expect("skill row must exist");
    row.get::<_, String>(0)
}

#[tokio::test]
async fn builtin_bootstrap_seed_lands_all_v3_components() {
    let Some(rig) = pg_rig_or_skip().await else {
        return;
    };
    let tenant = format!("t-{}", Uuid::new_v4());

    // First seed.
    seed_builtin_components(Arc::new(rig.pool.clone()), &tenant)
        .await
        .expect("seed must succeed");

    // 23 Tools (class 0) — the full first-party builtin tool set.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_tools", &tenant).await,
        23,
        "exactly 23 builtin tools"
    );
    // 30 ToolSkills (class 13).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant).await,
        30,
        "exactly 30 builtin tool skills"
    );
    // 84 PythonCodes (class 22) — tool executors + variants/helpers/gap-fillers
    // + the K4 host-fallback-prior-knowledge formatter.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_python_code", &tenant).await,
        84,
        "exactly 84 builtin python codes"
    );
    // 108 Skills (99 leaf class 1 + 9 domain class 2).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_skills", &tenant).await,
        108,
        "exactly 108 builtin skills (99 leaf + 9 domain)"
    );
    // 111 Recipes (class 21) — 15 Tier-0 + 2 Tier-1 in management + the rest
    // + the K4 host-assemble-prior-knowledge no-prefix fallback recipe.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_recipes", &tenant).await,
        111,
        "exactly 111 builtin recipes"
    );
    // 24 ExtensionCatalogues (class 23) — primary + per-tool ext catalogues.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_extension_catalogues", &tenant).await,
        24,
        "exactly 24 builtin extension catalogues"
    );

    // Safety-content regression guards.
    let spawn_content = tool_skill_content(&rig.pool, &tenant, "ts-spawn-subagent").await;
    assert!(
        spawn_content.to_lowercase().contains("scope isolation"),
        "ts-spawn-subagent ToolSkill content must carry the 'scope isolation' safety invariant"
    );
    let shell_safe_body = skill_body(&rig.pool, &tenant, "skill-shell-safe-check").await;
    assert!(
        shell_safe_body.to_lowercase().contains("approval"),
        "skill-shell-safe-check leaf skill body must carry the shell 'approval' safety rule"
    );

    // Idempotency: a re-seed leaves every count unchanged.
    let before = (
        count_validated_system(&rig.pool, "reborn_tools", &tenant).await,
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant).await,
        count_validated_system(&rig.pool, "reborn_python_code", &tenant).await,
        count_validated_system(&rig.pool, "reborn_skills", &tenant).await,
        count_validated_system(&rig.pool, "reborn_recipes", &tenant).await,
        count_validated_system(&rig.pool, "reborn_extension_catalogues", &tenant).await,
    );
    seed_builtin_components(Arc::new(rig.pool.clone()), &tenant)
        .await
        .expect("re-seed must succeed");
    let after = (
        count_validated_system(&rig.pool, "reborn_tools", &tenant).await,
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant).await,
        count_validated_system(&rig.pool, "reborn_python_code", &tenant).await,
        count_validated_system(&rig.pool, "reborn_skills", &tenant).await,
        count_validated_system(&rig.pool, "reborn_recipes", &tenant).await,
        count_validated_system(&rig.pool, "reborn_extension_catalogues", &tenant).await,
    );
    assert_eq!(before, after, "re-seed must be idempotent");
}
