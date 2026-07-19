//! Integration test: seed a libSQL DB, run migration, verify rows land in PG.
//!
//! This is the CI gate `seed_libsql_then_migrate_asserts_all_rows_in_pg` from
//! S12 of the upgrade plan. It requires a reachable PostgreSQL server via
//! `DATABASE_URL` or `BRASSCLAW_PG_URL`. Without a DB URL the test skips
//! (passing), matching the env-gated pattern used elsewhere.

#![cfg(feature = "migrate-from-libsql")]

use brassclaw_reborn_composition::migration;
use brassclaw_reborn_config::RebornHome;
use tempfile::tempdir;

/// Build a test Postgres pool from env. Returns `None` when no URL is set.
async fn test_pg_pool() -> Option<deadpool_postgres::Pool> {
    let url = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let config: tokio_postgres::Config = url.parse().ok()?;
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .ok()?;
    // Verify connectivity.
    let _ = pool.get().await.ok()?;
    Some(pool)
}

/// Seed a minimal libSQL database with one row in each migratable table,
/// run the migration, and assert the rows appear in Postgres.
#[tokio::test]
async fn seed_libsql_then_migrate_asserts_all_rows_in_pg() {
    let Some(pool) = test_pg_pool().await else {
        eprintln!(
            "seed_libsql_then_migrate_asserts_all_rows_in_pg: SKIPPED \
             (no DATABASE_URL / BRASSCLAW_PG_URL)"
        );
        return;
    };

    // Create an isolated Postgres schema for this test run.
    let schema = format!(
        "migration_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0)
    );
    {
        let client = pool.get().await.unwrap();
        client
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
            .await
            .unwrap();
        client
            .batch_execute(&format!("SET search_path TO {schema}"))
            .await
            .unwrap();
    }

    // Build a pool that always sets search_path to the test schema.
    let schema_for_hook = schema.clone();
    let config: tokio_postgres::Config = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap()
        .parse()
        .unwrap();
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let scoped_pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .post_create(deadpool_postgres::Hook::async_fn(move |client, _| {
            let s = schema_for_hook.clone();
            Box::pin(async move {
                client
                    .batch_execute(&format!("SET search_path TO {s}"))
                    .await
                    .map_err(|e| deadpool_postgres::HookError::message(e.to_string()))?;
                Ok(())
            })
        }))
        .build()
        .unwrap();

    // Run schema migrations so PG tables exist.
    brassclaw_pg::migrations::run_migrations(&scoped_pool)
        .await
        .expect("pg migrations failed");

    // Build a temp home dir with a minimal libSQL DB and config.toml.
    let home_dir = tempdir().unwrap();
    let home = RebornHome::resolve_from_env_parts(
        Some(home_dir.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();

    // Write a minimal config.toml.
    std::fs::write(
        home.config_file_path(),
        b"[identity]\ntenant = \"test-tenant\"\ndefault_owner = \"test-user\"\n",
    )
    .unwrap();

    // Write a minimal providers.json.
    std::fs::write(
        home.providers_file_path(),
        br#"[{"id":"test-provider","protocol":"openai","model_env":"TEST_MODEL","default_model":"gpt-4o"}]"#,
    )
    .unwrap();

    // Seed a minimal libSQL DB.
    seed_libsql_db(home.path()).await;

    // Run migration.
    let report = migration::run_migration(&scoped_pool, &home, "test-tenant", false)
        .await
        .expect("migration failed");

    assert!(report.config_migrated, "config.toml should have been migrated");
    assert!(report.providers_migrated, "providers.json should have been migrated");
    assert!(report.libsql_db_migrated, "reborn-local-dev.db should have been migrated");
    assert!(report.boot_initialized_set, "boot.initialized should have been set");

    // Verify rows landed in PG.
    let client = scoped_pool.get().await.unwrap();

    // config row
    let tenant_row: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_config WHERE tenant_id = 'test-tenant' AND key = 'identity.tenant'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(tenant_row, 1, "identity.tenant config row should be in PG");

    // boot.initialized
    let boot_row: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_config WHERE tenant_id = 'test-tenant' AND key = 'boot.initialized'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(boot_row, 1, "boot.initialized should be in PG");

    // provider row
    let provider_row: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_llm_providers WHERE tenant_id = 'test-tenant' AND id = 'test-provider'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(provider_row, 1, "provider row should be in PG");

    // safety_config row (seeded from libSQL)
    let safety_row: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_safety_config WHERE tenant_id = 'test-tenant'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(safety_row, 1, "safety_config row should be in PG");

    // trigger row (seeded from libSQL)
    let trigger_row: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_triggers WHERE tenant_id = 'test-tenant'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(trigger_row, 1, "trigger row should be in PG");

    // Source file renamed to .migrated
    assert!(
        !home.config_file_path().exists(),
        "config.toml should be renamed after migration"
    );
    assert!(
        home.path().join("config.toml.migrated").exists(),
        "config.toml.migrated should exist after migration"
    );
    assert!(
        !home.path().join("reborn-local-dev.db").exists(),
        "reborn-local-dev.db should be renamed after migration"
    );
    assert!(
        home.path().join("reborn-local-dev.db.migrated").exists(),
        "reborn-local-dev.db.migrated should exist after migration"
    );

    // Cleanup schema.
    {
        let client = pool.get().await.unwrap();
        let _ = client
            .execute(&format!("DROP SCHEMA {schema} CASCADE"), &[])
            .await;
    }
}

/// Dry-run migration asserts no files are renamed and no DB rows are written.
#[tokio::test]
async fn dry_run_migration_writes_nothing() {
    let Some(pool) = test_pg_pool().await else {
        eprintln!("dry_run_migration_writes_nothing: SKIPPED (no DB URL)");
        return;
    };

    let schema = format!(
        "migration_dry_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0)
    );
    {
        let client = pool.get().await.unwrap();
        client
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
            .await
            .unwrap();
    }
    let schema_for_hook = schema.clone();
    let config: tokio_postgres::Config = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap()
        .parse()
        .unwrap();
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let scoped_pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .post_create(deadpool_postgres::Hook::async_fn(move |client, _| {
            let s = schema_for_hook.clone();
            Box::pin(async move {
                client
                    .batch_execute(&format!("SET search_path TO {s}"))
                    .await
                    .map_err(|e| deadpool_postgres::HookError::message(e.to_string()))?;
                Ok(())
            })
        }))
        .build()
        .unwrap();
    brassclaw_pg::migrations::run_migrations(&scoped_pool)
        .await
        .expect("pg migrations");

    let home_dir = tempdir().unwrap();
    let home = RebornHome::resolve_from_env_parts(
        Some(home_dir.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    std::fs::write(
        home.config_file_path(),
        b"[identity]\ntenant = \"dry-tenant\"\n",
    )
    .unwrap();

    let report = migration::run_migration(&scoped_pool, &home, "dry-tenant", true)
        .await
        .expect("dry-run migration failed");

    // Dry-run: no files renamed.
    assert!(home.config_file_path().exists(), "config.toml should NOT be renamed in dry-run");
    assert!(!home.path().join("config.toml.migrated").exists());

    // Dry-run: nothing written to DB.
    let client = scoped_pool.get().await.unwrap();
    let row_count: i64 = client
        .query_one("SELECT COUNT(*) FROM brassclaw_config WHERE tenant_id = 'dry-tenant'", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(row_count, 0, "dry-run should write no config rows");

    // boot_initialized_set is false in dry-run (no write happened).
    assert!(!report.boot_initialized_set);

    // Cleanup.
    let client = pool.get().await.unwrap();
    let _ = client.execute(&format!("DROP SCHEMA {schema} CASCADE"), &[]).await;
}

// ---------------------------------------------------------------------------
// libSQL seeding helper
// ---------------------------------------------------------------------------

async fn seed_libsql_db(home_path: &std::path::Path) {
    let db_path = home_path.join("reborn-local-dev.db");
    let db = libsql::Builder::new_local(db_path.to_string_lossy().to_string())
        .build()
        .await
        .expect("build libsql db");
    let conn = db.connect().expect("connect libsql");

    // Create and seed the tables the migration reads.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS safety_config (key TEXT NOT NULL, value TEXT NOT NULL);
         INSERT INTO safety_config (key, value) VALUES ('some_rule', 'enabled');

         CREATE TABLE IF NOT EXISTS settings (section TEXT, key TEXT, value TEXT);

         CREATE TABLE IF NOT EXISTS memory_docs
             (id TEXT, path TEXT, content TEXT, metadata TEXT, created_at TEXT, updated_at TEXT);

         CREATE TABLE IF NOT EXISTS root_filesystem_entries
             (id TEXT, path TEXT, kind TEXT, content TEXT, metadata TEXT, created_at TEXT, updated_at TEXT);

         CREATE TABLE IF NOT EXISTS root_filesystem_index_specs
             (id TEXT, entry_id TEXT, kind TEXT, dimension INTEGER);

         CREATE TABLE IF NOT EXISTS root_filesystem_events
             (id TEXT, entry_id TEXT, kind TEXT, payload TEXT, created_at TEXT);

         CREATE TABLE IF NOT EXISTS capability_permissions
             (id TEXT, capability TEXT, effect TEXT, created_at TEXT);

         CREATE TABLE IF NOT EXISTS hooks_predicate_invocations
             (key_hash BLOB, scope_hash BLOB, event_id TEXT, recorded_at TEXT);

         CREATE TABLE IF NOT EXISTS hooks_predicate_values
             (key_hash BLOB, scope_hash BLOB, event_id TEXT, value TEXT, recorded_at TEXT);

         CREATE TABLE IF NOT EXISTS trigger_records
             (id TEXT, tenant_id TEXT, creator_user_id TEXT, name TEXT, description TEXT,
              trigger_kind TEXT, trigger_config TEXT, status TEXT, created_at TEXT, updated_at TEXT);
         INSERT INTO trigger_records
             (id, tenant_id, creator_user_id, name, description, trigger_kind, trigger_config, status, created_at, updated_at)
         VALUES ('tr-1', 'test-tenant', 'user-1', 'test', '', 'webhook', '{}', 'active', '', '');

         CREATE TABLE IF NOT EXISTS local_reborn_access
             (id TEXT, tenant_id TEXT, user_id TEXT, token_hash TEXT, created_at TEXT, updated_at TEXT);",
    )
    .await
    .expect("seed libsql db");
}
