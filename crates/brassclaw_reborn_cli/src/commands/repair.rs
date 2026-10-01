//! `brassclaw repair` — force-overwrite all `source='system'` component rows
//! from compiled-in seed values.
//!
//! Uses `ON CONFLICT DO UPDATE` — intentionally different from the normal
//! seeder's `DO NOTHING`. Run this after the boot content integrity check
//! fails to restore corrupted rows.

use clap::Args;

use crate::context::RebornCliContext;

/// Force-overwrite all `source='system'` component rows from compiled-in seed
/// constants.
///
/// The normal seeder uses `ON CONFLICT DO NOTHING` and will not overwrite a
/// corrupted row. This command unconditionally restores all system components
/// to their compiled-in state using `ON CONFLICT DO UPDATE`.
///
/// Run this when `brassclaw serve` fails with `CONTENT INTEGRITY FAILURE`.
#[derive(Debug, Args)]
pub(crate) struct RepairCommand {
    /// Simulate the repair without writing to the database.
    #[arg(long)]
    dry_run: bool,

    /// Tenant ID to repair. Defaults to the value in config.toml
    /// (`identity.tenant`), then `"default"`.
    #[arg(long)]
    tenant: Option<String>,
}

impl RepairCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        #[cfg(not(feature = "postgres"))]
        {
            let _ = context;
            anyhow::bail!(
                "the repair command requires the `postgres` feature to be enabled"
            );
        }

        #[cfg(feature = "postgres")]
        {
            use std::sync::Arc;

            use brassclaw_reborn_composition::booted_db::run_migrations_and_return_booted_db;
            use brassclaw_reborn_composition::repair::repair_builtin_components;

            let home = context.boot_config().home().clone();

            // Resolve tenant_id: CLI flag → config.toml → "default"
            let tenant_id = self.tenant.clone().unwrap_or_else(|| {
                brassclaw_reborn_config::RebornConfigFile::load(
                    &home.path().join("config.toml"),
                )
                .ok()
                .flatten()
                .and_then(|c| c.identity)
                .and_then(|id| id.tenant)
                .unwrap_or_else(|| "default".to_string())
            });

            // Build and connect to Postgres.
            let pool = build_pg_pool().await?;
            let pool = Arc::new(pool);

            // Run migrations and get the BootedDb token.
            let booted_db = run_migrations_and_return_booted_db(pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            let mode = if self.dry_run { "dry-run" } else { "live" };
            println!("brassclaw repair [{mode}]: tenant={tenant_id}");

            if self.dry_run {
                println!("  (no writes will be performed)");
            }

            let report =
                repair_builtin_components(&booted_db, &tenant_id, self.dry_run)
                    .await
                    .map_err(|e| anyhow::anyhow!("repair failed: {e}"))?;

            if self.dry_run {
                println!("  Would restore {} system component(s).", report.restored);
                println!("[dry-run] no writes performed");
            } else {
                println!("  Restored {} system component(s).", report.restored);
            }

            Ok(())
        }
    }
}

/// Build a Postgres pool for the repair command.
async fn build_pg_pool() -> anyhow::Result<deadpool_postgres::Pool> {
    crate::commands::config::pg_lifecycle::build_pg_pool().await
}
