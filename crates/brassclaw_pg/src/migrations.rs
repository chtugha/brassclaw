use deadpool_postgres::Pool;
use tracing::debug;

use crate::error::PgError;

refinery::embed_migrations!("migrations");

/// Run all pending schema migrations against the given pool.
///
/// Before running `refinery`, this function checks whether the DB is a
/// pre-existing deployment that has tables created outside of refinery
/// (via `CREATE TABLE IF NOT EXISTS` in inline DDL). If so, it inserts
/// pre-seeded history rows so refinery does not attempt to re-apply those
/// migrations.
///
/// All migration files use `CREATE TABLE IF NOT EXISTS` and
/// `CREATE INDEX IF NOT EXISTS` so they are safe to re-run in edge cases.
pub async fn run_migrations(pool: &Pool) -> Result<(), PgError> {
    let mut client = pool.get().await?;

    // Reconcile history for pre-refinery deployments.
    reconcile_history(&client).await?;

    // Run the refinery migration runner.
    let report = migrations::runner()
        .run_async(&mut **client)
        .await
        .map_err(|e| PgError::Migration(e.to_string()))?;

    let applied = report.applied_migrations();
    if applied.is_empty() {
        debug!("all migrations already applied; schema is up to date");
    } else {
        for m in applied {
            debug!(version = m.version(), name = m.name(), "applied migration");
        }
    }

    Ok(())
}

/// Pre-seed refinery history rows for tables that existed before refinery was
/// introduced. This prevents refinery from re-applying migrations whose SQL
/// already ran via the old `CREATE TABLE IF NOT EXISTS` inline DDL path.
///
/// The check is: if `refinery_schema_history` is empty (fresh refinery install)
/// AND any of the known pre-existing tables exist, insert synthetic history rows
/// marking those migrations as already applied.
async fn reconcile_history(
    client: &deadpool_postgres::Client,
) -> Result<(), PgError> {
    // Check if refinery history table exists and is empty.
    let history_exists: bool = client
        .query_one(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = 'refinery_schema_history'
                  AND table_schema = current_schema()
            )",
            &[],
        )
        .await
        .map(|row| row.get::<_, bool>(0))
        .unwrap_or(false);

    if history_exists {
        // Refinery has already been run; no pre-seeding needed.
        return Ok(());
    }

    // Check for pre-existing tables created outside refinery.
    let pre_existing = detect_pre_existing_tables(client).await?;
    if pre_existing.is_empty() {
        return Ok(());
    }

    debug!(
        tables = ?pre_existing,
        "detected pre-existing tables; pre-seeding refinery history"
    );

    // Create the history table if it does not exist yet (refinery creates it
    // on first run, but we need it before the runner executes).
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS refinery_schema_history (
                version     INT         NOT NULL PRIMARY KEY,
                name        TEXT        NOT NULL,
                applied_on  TEXT        NOT NULL,
                checksum    TEXT        NOT NULL
            )",
        )
        .await
        .map_err(|e| PgError::Migration(e.to_string()))?;

    // Insert placeholder rows for all pre-refinery migration versions.
    // The checksum value "pre-seeded" is a marker that this row was
    // synthesised, not applied by refinery. Refinery will not re-run
    // a version that already has a history row.
    for (version, name) in pre_existing {
        client
            .execute(
                "INSERT INTO refinery_schema_history (version, name, applied_on, checksum)
                 VALUES ($1, $2, NOW()::TEXT, 'pre-seeded')
                 ON CONFLICT (version) DO NOTHING",
                &[&version, &name],
            )
            .await
            .map_err(|e| PgError::Migration(e.to_string()))?;
    }

    Ok(())
}

/// Detect tables that were created by the old inline-DDL path (outside refinery).
/// Returns a list of `(migration_version, migration_name)` pairs to pre-seed.
async fn detect_pre_existing_tables(
    client: &deadpool_postgres::Client,
) -> Result<Vec<(i32, &'static str)>, PgError> {
    let table_check = |name: &str| {
        format!(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = '{name}'
                  AND table_schema = current_schema()
            )"
        )
    };

    let mut pre_existing = Vec::new();

    // hooks tables — created by brassclaw_hooks_pg inline DDL
    let hooks_exist: bool = client
        .query_one(&table_check("hooks_predicate_invocations"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    if hooks_exist {
        pre_existing.push((17_i32, "hooks"));
    }

    // trigger_records — the pre-V021 table name created outside refinery.
    // V021 renames it to brassclaw_triggers; both names are checked here.
    let triggers_exist: bool = client
        .query_one(&table_check("trigger_records"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    // Also check brassclaw_triggers (already renamed)
    let brassclaw_triggers_exist: bool = client
        .query_one(&table_check("brassclaw_triggers"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    if triggers_exist || brassclaw_triggers_exist {
        pre_existing.push((21_i32, "triggers"));
    }

    // settings table (libSQL DbTokenSettingsStore)
    let settings_exist: bool = client
        .query_one(&table_check("settings"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    if settings_exist {
        pre_existing.push((14_i32, "token_settings"));
    }

    // safety_config table
    let safety_exist: bool = client
        .query_one(&table_check("safety_config"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    if safety_exist {
        pre_existing.push((15_i32, "safety"));
    }

    // VFS backing tables — all three are created together by PostgresRootFilesystem::run_migrations
    // outside refinery. Revision 18 H1 fix: checking only root_filesystem_entries missed the two
    // sibling tables (root_filesystem_index_specs, root_filesystem_events). If ANY of the three
    // is present, the entire V018 bundle has been applied and must be pre-seeded.
    let vfs_entries_exist: bool = client
        .query_one(&table_check("root_filesystem_entries"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    let vfs_index_specs_exist: bool = client
        .query_one(&table_check("root_filesystem_index_specs"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    let vfs_events_exist: bool = client
        .query_one(&table_check("root_filesystem_events"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    if vfs_entries_exist || vfs_index_specs_exist || vfs_events_exist {
        pre_existing.push((18_i32, "root_filesystem"));
    }

    // memory_docs table
    let memory_docs_exist: bool = client
        .query_one(&table_check("memory_docs"), &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(false);
    if memory_docs_exist {
        pre_existing.push((16_i32, "memory_docs"));
    }

    Ok(pre_existing)
}

#[cfg(test)]
mod tests {
    // Integration tests that require a live Postgres are gated behind the
    // `integration` feature to avoid running in the standard `cargo test` pass.
    // Run them with: cargo test -p brassclaw_pg --features integration

    #[cfg(feature = "integration")]
    mod integration {
        use super::super::*;
        use crate::pool::build_pool;

        #[tokio::test]
        async fn fresh_db_gets_all_tables() {
            let pg_url = std::env::var("TEST_PG_URL")
                .unwrap_or_else(|_| "postgresql://brassclaw@127.0.0.1:5434/brassclaw".to_string());
            let pool = build_pool(&pg_url).expect("build pool");
            run_migrations(&pool).await.expect("migrations");

            let client = pool.get().await.expect("get client");
            for table in [
                "brassclaw_config",
                "brassclaw_llm_providers",
                "brassclaw_secrets_master",
                "brassclaw_secrets",
                "brassclaw_runs",
                "brassclaw_approvals",
                "brassclaw_turns",
                "brassclaw_capability_leases",
                "brassclaw_session_threads",
                "brassclaw_processes",
                "brassclaw_process_results",
                "brassclaw_extension_manifests",
                "brassclaw_extensions",
                "brassclaw_resource_accounts",
                "brassclaw_checkpoints",
                "brassclaw_events",
                "brassclaw_audit_log",
                "brassclaw_token_settings",
                "brassclaw_safety_config",
                "brassclaw_capability_permissions",
                "brassclaw_memory_docs",
                "hooks_predicate_invocations",
                "hooks_predicate_values",
                "brassclaw_root_filesystem",
                "brassclaw_root_filesystem_index_specs",
                "brassclaw_root_filesystem_events",
                "brassclaw_budget_gates",
                "brassclaw_identities",
                "brassclaw_identity_users",
                "brassclaw_identity_email_index",
                "brassclaw_triggers",
                "brassclaw_local_access",
                "brassclaw_conversation_state",
                "brassclaw_outbound_policies",
                "brassclaw_outbound_subscriptions",
                "brassclaw_outbound_deliveries",
                "brassclaw_outbound_preferences",
                "brassclaw_subagent_goals",
                "brassclaw_memory_chat_records",
                "brassclaw_forensic_packets",
            ] {
                let row = client
                    .query_one(
                        "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
                         WHERE table_name = $1 AND table_schema = current_schema())",
                        &[&table],
                    )
                    .await
                    .unwrap();
                let exists: bool = row.get(0);
                assert!(exists, "table {table} not found after migration");
            }
        }

        #[tokio::test]
        async fn migration_is_idempotent() {
            let pg_url = std::env::var("TEST_PG_URL")
                .unwrap_or_else(|_| "postgresql://brassclaw@127.0.0.1:5434/brassclaw".to_string());
            let pool = build_pool(&pg_url).expect("build pool");
            // Run twice — must not fail.
            run_migrations(&pool).await.expect("first run");
            run_migrations(&pool).await.expect("second run (idempotent)");
        }

        /// Verify that a DB that already has the hooks tables (created by the old
        /// inline-DDL path in `brassclaw_hooks_pg`) does not cause refinery to
        /// re-apply V017 and fail with "table already exists".
        ///
        /// Approach: create the hooks tables manually, then run migrations and
        /// assert no error. The reconcile_history pre-seeding must insert a V017
        /// history row so refinery skips the migration entirely.
        #[tokio::test]
        async fn pre_existing_hooks_tables_do_not_trip_refinery() {
            let pg_url = std::env::var("TEST_PG_URL")
                .unwrap_or_else(|_| "postgresql://brassclaw@127.0.0.1:5434/brassclaw_test_parity".to_string());
            let pool = build_pool(&pg_url).expect("build pool");

            // Pre-create the hooks tables exactly as brassclaw_hooks_pg would.
            let client = pool.get().await.expect("get client");
            client
                .batch_execute(
                    "CREATE TABLE IF NOT EXISTS hooks_predicate_invocations (
                        id TEXT NOT NULL PRIMARY KEY,
                        tenant_id TEXT NOT NULL,
                        hook_id TEXT NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
                    );
                    CREATE TABLE IF NOT EXISTS hooks_predicate_values (
                        id TEXT NOT NULL PRIMARY KEY,
                        invocation_id TEXT NOT NULL,
                        key TEXT NOT NULL,
                        value TEXT NOT NULL
                    );",
                )
                .await
                .expect("seed hooks tables");
            drop(client);

            // Running migrations must not fail even though V017 tables already exist.
            run_migrations(&pool).await.expect("migrations must not fail on pre-existing hooks tables");

            // Verify V017 history row was inserted by reconcile_history (not by refinery runner).
            let client = pool.get().await.expect("get client");
            let row = client
                .query_one(
                    "SELECT checksum FROM refinery_schema_history WHERE version = 17",
                    &[],
                )
                .await
                .expect("V017 history row must exist");
            let checksum: String = row.get(0);
            assert_eq!(
                checksum, "pre-seeded",
                "V017 row must be pre-seeded (not applied by refinery runner)"
            );
        }

        /// S2 prerequisite test: verify the `vector` extension is installed by V000
        /// and that the pgvector `<=>` cosine-distance operator is available.
        ///
        /// This is the §4.30.3 prerequisite: the chunk system's `embedding` indexed key
        /// is stored as a `vector(N)` column in the VFS backing table, and
        /// `Filter::VectorNearest` translates to a pgvector `<=>` cosine query in
        /// `PostgresRootFilesystem`. The extension must be available after migrations run.
        #[tokio::test]
        async fn pgvector_extension_available_and_cosine_operator_works() {
            let pg_url = std::env::var("TEST_PG_URL")
                .unwrap_or_else(|_| "postgresql://brassclaw@127.0.0.1:5434/brassclaw".to_string());
            let pool = build_pool(&pg_url).expect("build pool");
            run_migrations(&pool).await.expect("migrations");

            let client = pool.get().await.expect("get client");

            // Verify the vector extension is registered.
            let row = client
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector')",
                    &[],
                )
                .await
                .expect("pg_extension query");
            let ext_installed: bool = row.get(0);
            assert!(ext_installed, "pgvector extension must be installed by V000");

            // Verify the <=> cosine-distance operator works: cast literal arrays to vector
            // and compute cosine distance. This exercises the same operator that
            // PostgresRootFilesystem uses for Filter::VectorNearest queries.
            // '[1,0,0]'::vector <=> '[1,0,0]'::vector = 0.0 (identical vectors, zero distance).
            let row = client
                .query_one(
                    "SELECT ('[1,0,0]'::vector <=> '[1,0,0]'::vector)::float4",
                    &[],
                )
                .await
                .expect("pgvector <=> operator must be available after V000");
            let cosine_distance: f32 = row.get(0);
            assert!(
                cosine_distance.abs() < 1e-5,
                "cosine distance of identical vectors must be ~0.0, got {cosine_distance}"
            );

            // Verify orthogonal vectors have distance ~1.0.
            let row = client
                .query_one(
                    "SELECT ('[1,0,0]'::vector <=> '[0,1,0]'::vector)::float4",
                    &[],
                )
                .await
                .expect("pgvector <=> operator must work for orthogonal vectors");
            let cosine_distance_orth: f32 = row.get(0);
            assert!(
                (cosine_distance_orth - 1.0_f32).abs() < 1e-5,
                "cosine distance of orthogonal vectors must be ~1.0, got {cosine_distance_orth}"
            );
        }
    }
}
