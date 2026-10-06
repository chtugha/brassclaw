//!
//! Phase L integration test — the v3 builtin-tool component seed
//! ([`brassclaw_reborn_composition::builtin_bootstrap::seed_builtin_components`]).
//!
//! Starts an isolated Postgres-16 testcontainer, runs the full migration set,
//! calls both installation seeders in production order and asserts the complete
//! host + capability stack, including Pass 17 prefix tools, lands as
//! `source = "system"` + `validation_status = "validated"` rows. It also checks
//! every component table on fresh install and after reseeding a set of legacy
//! pending system rows. User-authored pending rows must remain pending.
//! Re-runs the seed to prove idempotency (counts unchanged). Also guards the
//! safety-critical content: `ts-spawn-subagent` carries "scope isolation" and
//! the `skill-shell-safe-check` skill body carries the "approval" rule.
//! Returns early (pass) when docker/testcontainers is unavailable. Gated to
//! `skills-db` (which implies `postgres`) because the seed + stores are
//! postgres-only.
//!
//! This checks the seed's complete row set and idempotency independently of
//! runtime boot. `component_boot.rs` now propagates required seeding errors
//! before any runtime worker starts.

#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use brassclaw_reborn_composition::booted_db::BootedDb;
use brassclaw_reborn_composition::builtin_bootstrap::seed_builtin_components;
use brassclaw_reborn_composition::seed_builtin_host::seed_builtin_host_components;
use tokio_postgres::types::ToSql;
use uuid::Uuid;

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: PgPool,
    booted_db: BootedDb,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration. Returns `None` (skip) when docker is unavailable.
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
    let booted_db = brassclaw_reborn_composition::booted_db::run_migrations_and_return_booted_db(
        Arc::new(pool.clone()),
    )
    .await
    .expect("migrations must apply before boot seeding");
    Some(PgRig {
        _container: container,
        pool,
        booted_db,
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
    let row = client.query_one(&sql, params).await.expect("count query");
    row.get(0)
}

async fn count_pending_system(pool: &PgPool, table: &str, tenant: &str) -> i64 {
    let client = pool.get().await.expect("pool client");
    let user_id = SYSTEM_RESERVED_ID.to_string();
    let sql = format!(
        "SELECT COUNT(*) FROM {table}
         WHERE tenant_id = $1 AND user_id = $2 AND agent_id = 'default'
           AND project_id = 'system' AND source = 'system'
           AND validation_status != 'validated'"
    );
    client
        .query_one(&sql, &[&tenant, &user_id])
        .await
        .expect("pending seeds query")
        .get(0)
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

    // Exercise the same full installation seed order as WebUI composition.
    seed_builtin_host_components(&rig.booted_db, &tenant)
        .await
        .expect("host seed must succeed");
    seed_builtin_components(&rig.booted_db, &tenant)
        .await
        .expect("seed must succeed");

    // Host infrastructure + capability tools, including Pass 17.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_tools", &tenant).await,
        33,
        "exactly 33 host and capability tools"
    );
    // 30 ToolSkills (class 13).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_tool_skills", &tenant).await,
        40,
        "exactly 40 builtin tool skills"
    );
    // 92 PythonCodes (class 22) — 84 tool executors + variants/helpers/gap-fillers
    // + the K4 host-fallback-prior-knowledge formatter
    // + 8 validator PythonCodes (classes 0, 1, 2, 3, 13, 21, 22, 23 — Phase L §0.23.3).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_python_code", &tenant).await,
        105,
        "exactly 105 builtin python codes across host and capability seeds"
    );
    // 108 Skills (99 leaf class 1 + 9 domain class 2).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_skills", &tenant).await,
        118,
        "exactly 118 builtin skills across host and capability seeds"
    );
    // 119 Recipes (class 21) — 111 domain Recipes
    // + 8 validator Recipes (classes 0, 1, 2, 3, 13, 21, 22, 23 — Phase L §0.23.3).
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_recipes", &tenant).await,
        126,
        "exactly 126 builtin recipes across host and capability seeds"
    );
    // 24 ExtensionCatalogues (class 23) — primary + per-tool ext catalogues.
    assert_eq!(
        count_validated_system(&rig.pool, "reborn_extension_catalogues", &tenant).await,
        26,
        "exactly 26 builtin extension catalogues"
    );

    // A pending authored row in the seed identity scope must not be promoted
    // by the installation-only status repair.
    let authored_name = format!("authored-pending-{}", Uuid::new_v4());
    let client = rig.pool.get().await.expect("pool client");
    let reserved_user = SYSTEM_RESERVED_ID.to_string();
    client.execute(
        "INSERT INTO reborn_skills
            (tenant_id,user_id,agent_id,project_id,name,description,body,class_code,source,validation_status)
         VALUES ($1,$2,'default','system',$3,'authored fixture','must remain pending',1,'authored','pending')",
        &[&tenant, &reserved_user, &authored_name],
    ).await.expect("insert authored pending fixture");

    // A legacy pending installation seed can be promoted only when its live
    // bytes hash to the compiled-in expected checksum.
    client
        .execute(
            "UPDATE reborn_skills SET validation_status='pending'
          WHERE tenant_id=$1 AND user_id=$2 AND agent_id='default' AND project_id='system'
            AND name='orchestrator:main' AND source='system'",
            &[&tenant, &reserved_user],
        )
        .await
        .expect("degrade known checksummed prompt");
    seed_builtin_components(&rig.booted_db, &tenant)
        .await
        .expect("exact compiled-in checksum permits legacy status repair");
    let repaired_status: String = client
        .query_one(
            "SELECT validation_status FROM reborn_skills
          WHERE tenant_id=$1 AND user_id=$2 AND agent_id='default' AND project_id='system'
            AND name='orchestrator:main' AND source='system'",
            &[&tenant, &reserved_user],
        )
        .await
        .expect("known prompt remains")
        .get(0);
    assert_eq!(repaired_status, "validated");

    // A changed row is a possible operator upgrade: reseeding must fail closed
    // and preserve the changed body/status until an explicit repair decision.
    let original: tokio_postgres::Row = client
        .query_one(
            "SELECT body, content_checksum FROM reborn_skills
          WHERE tenant_id=$1 AND user_id=$2 AND agent_id='default' AND project_id='system'
            AND name='codeact_preamble' AND source='system'",
            &[&tenant, &reserved_user],
        )
        .await
        .expect("known prompt exists");
    let original_body: String = original.get(0);
    let original_checksum: Option<String> = original.get(1);
    let upgraded_body = format!("{original_body}\noperator upgrade");
    client
        .execute(
            "UPDATE reborn_skills SET body=$3, validation_status='pending'
          WHERE tenant_id=$1 AND user_id=$2 AND agent_id='default' AND project_id='system'
            AND name='codeact_preamble' AND source='system'",
            &[&tenant, &reserved_user, &upgraded_body],
        )
        .await
        .expect("simulate modified installation content");
    assert!(
        seed_builtin_components(&rig.booted_db, &tenant)
            .await
            .is_err()
    );
    let preserved: (String, String) = {
        let row = client
            .query_one(
                "SELECT body, validation_status FROM reborn_skills
              WHERE tenant_id=$1 AND user_id=$2 AND agent_id='default' AND project_id='system'
                AND name='codeact_preamble' AND source='system'",
                &[&tenant, &reserved_user],
            )
            .await
            .expect("modified prompt remains");
        (row.get(0), row.get(1))
    };
    assert_eq!(preserved, (upgraded_body, "pending".to_string()));
    client
        .execute(
            "UPDATE reborn_skills SET body=$3, content_checksum=$4, validation_status='validated'
          WHERE tenant_id=$1 AND user_id=$2 AND agent_id='default' AND project_id='system'
            AND name='codeact_preamble' AND source='system'",
            &[&tenant, &reserved_user, &original_body, &original_checksum],
        )
        .await
        .expect("restore fixture state");

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

    let all_component_tables = [
        "reborn_skills",
        "reborn_tools",
        "reborn_actions",
        "reborn_specs",
        "reborn_summaries",
        "reborn_lessons",
        "reborn_issues",
        "reborn_notes",
        "reborn_tool_skills",
        "reborn_plans",
        "reborn_docus",
        "reborn_python_code",
        "reborn_recipes",
        "reborn_extensions_unified",
        "reborn_extension_catalogues",
    ];
    for table in all_component_tables {
        assert_eq!(
            count_pending_system(&rig.pool, table, &tenant).await,
            0,
            "fresh installation must seed every system component as validated ({table})"
        );
    }
    seed_builtin_components(&rig.booted_db, &tenant)
        .await
        .expect("reseed must leave seeded component validation unchanged");
    for table in all_component_tables {
        assert_eq!(
            count_pending_system(&rig.pool, table, &tenant).await,
            0,
            "every fresh installation seed must remain validated in {table}"
        );
    }

    let authored_status: String = rig.pool.get().await.expect("pool client").query_one(
        "SELECT validation_status FROM reborn_skills WHERE tenant_id=$1 AND user_id=$2 AND name=$3",
        &[&tenant, &reserved_user, &authored_name],
    ).await.expect("authored fixture remains").get(0);
    assert_eq!(
        authored_status, "pending",
        "system seed repair must not validate authored rows"
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
    seed_builtin_components(&rig.booted_db, &tenant)
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
