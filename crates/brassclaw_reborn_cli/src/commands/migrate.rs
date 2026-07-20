use clap::Args;

use crate::context::RebornCliContext;

/// Migrate local state from libSQL / file-based storage to PostgreSQL.
///
/// Runs §8.1 steps 3–8 of the upgrade migration. Idempotent: safe to re-run
/// after an interrupted migration.
///
/// Requires a running PostgreSQL instance (embedded or external) and the
/// schema migrations to have been applied first (`brassclaw serve` does this
/// automatically; alternatively run `brassclaw serve --dry-run`).
#[derive(Debug, Args)]
pub(crate) struct MigrateCommand {
    /// Read-only simulation mode: print what would be migrated without writing
    /// to the database or renaming any files.
    #[arg(long)]
    dry_run: bool,

    /// Tenant ID to migrate data under. Defaults to the value in config.toml
    /// (`identity.tenant`), then `"default"`.
    #[arg(long)]
    tenant: Option<String>,
}

impl MigrateCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        #[cfg(not(feature = "migrate-from-libsql"))]
        {
            let _ = context;
            let _ = self;
            anyhow::bail!(
                "the migrate command requires the `migrate-from-libsql` feature to be enabled"
            );
        }

        #[cfg(feature = "migrate-from-libsql")]
        {
            use brassclaw_reborn_composition::migration;

            let home = context.boot_config().home().clone();

            // Resolve tenant_id: CLI flag → config.toml → "default"
            let tenant_id = self.tenant.clone().unwrap_or_else(|| {
                brassclaw_reborn_config::RebornConfigFile::load(&home.path().join("config.toml"))
                    .ok()
                    .flatten()
                    .and_then(|c| c.identity)
                    .and_then(|id| id.tenant)
                    .unwrap_or_else(|| "default".to_string())
            });

            // Build a Postgres pool. Uses BRASSCLAW_PG_URL or starts embedded PG.
            let pool = build_pg_pool(&context).await?;

            let mode = if self.dry_run { "dry-run" } else { "live" };
            println!("brassclaw migrate [{mode}]: tenant={tenant_id}");

            let report = migration::run_migration(&pool, &home, &tenant_id, self.dry_run)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            println!("  config.toml      : {}", status(report.config_migrated));
            println!("  providers.json   : {}", status(report.providers_migrated));
            println!("  sempai_provider  : {}", status(report.sempai_migrated));
            println!(
                "  secrets master   : {}",
                status(report.secrets_master_migrated)
            );
            println!("  reborn-local-dev : {}", status(report.libsql_db_migrated));
            if report.boot_initialized_set {
                println!("  boot.initialized : set");
            }
            if !report.any_migrated() {
                println!("  (no source artifacts found — nothing to migrate)");
            }
            if self.dry_run {
                println!("\n[dry-run] no writes performed");
            }

            Ok(())
        }
    }
}

fn status(migrated: bool) -> &'static str {
    if migrated {
        "migrated"
    } else {
        "not found (skipped)"
    }
}

/// Build a Postgres pool for the migrate command. Uses `BRASSCLAW_PG_URL` if
/// set, otherwise reports an error asking the user to start `brassclaw serve`
/// first (which starts embedded PG automatically).
async fn build_pg_pool(_context: &RebornCliContext) -> anyhow::Result<deadpool_postgres::Pool> {
    let url = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| {
            anyhow::anyhow!(
                "no PostgreSQL URL found; set BRASSCLAW_PG_URL or start `brassclaw serve` \
             first to have embedded PostgreSQL started automatically"
            )
        })?;

    let config: tokio_postgres::Config = url
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid PostgreSQL URL: {e}"))?;
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build PG pool: {e}"))?;

    // Verify connectivity.
    let _client = pool
        .get()
        .await
        .map_err(|e| anyhow::anyhow!("cannot connect to PostgreSQL: {e}"))?;

    Ok(pool)
}
