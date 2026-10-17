//!
//! Phase L integration test — the v3 builtin-tool component seed
//! ([`brassclaw_reborn_composition::builtin_bootstrap::seed_builtin_components`]).
//!
//! Starts the bundled verified PostgreSQL server in a private directory, runs the full migration set,
//! calls both installation seeders in production order and asserts the complete
//! host + capability stack, including Pass 17 prefix tools, lands as
//! `source = "system"` + `validation_status = "validated"` rows. It also checks
//! every component table on fresh install and after reseeding a set of legacy
//! pending system rows. User-authored pending rows must remain pending.
//! Re-runs the seed to prove idempotency (counts unchanged). Also guards the
//! safety-critical content: `ts-spawn-subagent` carries "scope isolation" and
//! the `skill-shell-safe-check` skill body carries the "approval" rule.
//! Initialization failures fail the test. Gated to
//! `skills-db` (which implies `postgres`) because the seed + stores are
//! postgres-only.
//!
//! This checks the seed's complete row set and idempotency independently of
//! runtime boot. `component_boot.rs` now propagates required seeding errors
//! before any runtime worker starts.

#![cfg(feature = "skills-db")]

#[path = "common/native_pg.rs"]
mod native_pg;

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use brassclaw_reborn_composition::booted_db::BootedDb;
use brassclaw_reborn_composition::builtin_bootstrap::seed_builtin_components;
use brassclaw_reborn_composition::seed_builtin_host::seed_builtin_host_components;
use secrecy::ExposeSecret;
use tokio_postgres::types::ToSql;
use uuid::Uuid;

struct PgRig {
    pool: PgPool,
    booted_db: BootedDb,
    // Stop the owning server only after both pool handles above are released.
    _server: Arc<native_pg::NativePostgres>,
}

async fn pg_rig() -> PgRig {
    let server = Arc::new(native_pg::NativePostgres::start().await);
    let booted_db = brassclaw_reborn_composition::booted_db::run_migrations_and_return_booted_db(
        server.pool.clone(),
    )
    .await
    .expect("migrations and their idempotent readback must succeed");
    PgRig {
        pool: (*server.pool).clone(),
        booted_db,
        _server: server,
    }
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
    let rig = pg_rig().await;
    let tenant = format!("t-{}", Uuid::new_v4());

    // Exercise the same full installation seed order as WebUI composition.
    seed_builtin_host_components(&rig.booted_db, &tenant)
        .await
        .expect("host seed must succeed");
    seed_builtin_components(&rig.booted_db, &tenant)
        .await
        .expect("seed must succeed");
    let initial = rig.pool.get().await.unwrap();
    let legacy_source: serde_json::Value = initial.query_one(
        "SELECT jsonb_agg(jsonb_build_object('id',id,'content',content,'checksum',content_checksum) ORDER BY id)
         FROM reborn_python_code WHERE tenant_id=$1 AND name LIKE 'pc-validator-class-%'",
        &[&tenant]).await.unwrap().get(0);
    assert_eq!(legacy_source.as_array().unwrap().len(), 8);
    let row = initial.query_one(
        "SELECT component_id,version FROM reborn_component_revisions
         WHERE class_code=22 AND revision_bytes::jsonb->'document'->>'name'='check-structural-presence'",
        &[]).await.unwrap();
    let (presence_id, presence_version): (Uuid, i64) = (row.get(0), row.get(1));
    drop(initial);
    let revisions =
        brassclaw_skills::revision_store::PgComponentRevisionStore::new(Arc::new(rig.pool.clone()));
    let presence_original = revisions
        .read_revision(presence_id, presence_version as u64)
        .await
        .unwrap();
    assert_eq!(
        presence_original.draft().document()["content"],
        brassclaw_skills::validator_components::STRUCTURAL_PRESENCE_SOURCE
    );
    let mut authored: serde_json::Value =
        serde_json::from_str(presence_original.draft().exact_bytes()).unwrap();
    authored["document"]["description"] =
        serde_json::json!("Retained operator draft, not an active validator");
    let authored = brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
        &authored.to_string(),
    )
    .unwrap();
    let authored = revisions
        .stage(&authored, presence_version as u64)
        .await
        .unwrap();

    // Reviewed installation inventory: 8 host primitives, 21 capability
    // primitives, 2 prefix primitives and the Zencoder primitive. History
    // persistence reuses memory_write; it is not a separate host Tool.
    let expected_tools = [
        "apply_patch",
        "component_db",
        "echo",
        "glob",
        "grep",
        "host.check_signals",
        "host.compose_orchestrator",
        "host.fetch_component",
        "host.kohai_complete",
        "host.post_reply",
        "host.resolve_component_by_name",
        "host.resolve_intent",
        "host.store_prefix_bundle",
        "host.sweep_validated_components",
        "host.validate_component",
        "http",
        "http.save",
        "json",
        "list_dir",
        "memory_read",
        "memory_search",
        "memory_tree",
        "memory_write",
        "read_file",
        "shell",
        "spawn_subagent",
        "time",
        "trigger_create",
        "trigger_list",
        "trigger_remove",
        "write_file",
        "zencoder-api",
    ];
    let inventory_client = rig.pool.get().await.expect("inventory client");
    let user = SYSTEM_RESERVED_ID.to_string();
    let tools: Vec<String> = inventory_client
        .query(
            "SELECT name FROM reborn_tools WHERE tenant_id=$1 AND user_id=$2
         AND agent_id='default' AND project_id='system' AND source='system'
         AND validation_status='validated' ORDER BY name COLLATE \"C\"",
            &[&tenant, &user],
        )
        .await
        .expect("installation tool inventory")
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(tools, expected_tools);
    drop(inventory_client);
    let tables = [
        "reborn_tools",
        "reborn_tool_skills",
        "reborn_python_code",
        "reborn_skills",
        "reborn_recipes",
        "reborn_extension_catalogues",
    ];
    let mut counts = Vec::new();
    for table in tables {
        counts.push(count_validated_system(&rig.pool, table, &tenant).await);
    }
    assert_eq!(
        counts,
        [32, 46, 134, 179, 151, 34],
        "complete installation inventory: {tables:?}"
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
    assert_eq!(
        revisions.read_allocation_head(presence_id).await.unwrap(),
        Some((22, authored.version)),
        "reseeding must not replace the authored head"
    );
    assert_eq!(
        revisions
            .read_revision(presence_id, presence_version as u64)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        presence_original.draft().exact_bytes()
    );
    let preserved: serde_json::Value = rig.pool.get().await.unwrap().query_one(
        "SELECT jsonb_agg(jsonb_build_object('id',id,'content',content,'checksum',content_checksum) ORDER BY id)
         FROM reborn_python_code WHERE tenant_id=$1 AND name LIKE 'pc-validator-class-%'",
        &[&tenant]).await.unwrap().get(0);
    assert_eq!(
        preserved, legacy_source,
        "all eight legacy bodies and checksums stay unchanged"
    );
}

/// Actual PostgreSQL rejection through refinery, not an invented error value.
#[tokio::test]
async fn migration_rejection_preserves_safe_database_classification() {
    let rig = native_pg::NativePostgres::start().await;
    let url = format!(
        "{}?options=-c%20default_transaction_read_only%3Don",
        rig.url.expose_secret()
    );
    let pool = brassclaw_pg::pool::build_pool_with_max_size(&url, 1).unwrap();
    let client = pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SHOW default_transaction_read_only", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "on"
    );
    drop(client);
    let error = brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, brassclaw_pg::PgError::Migration(reason) if reason.contains("SQLSTATE 25006")),
        "read-only server rejection must retain its safe classification: {error}"
    );
    pool.close();
    drop(pool);
    drop(rig);
}
