use clap::{Args, Subcommand};

use crate::context::RebornCliContext;

#[derive(Debug, Args)]
pub(crate) struct MaintenanceCommand {
    #[command(subcommand)]
    subcommand: MaintenanceSubcommand,
}

#[derive(Debug, Subcommand)]
enum MaintenanceSubcommand {
    /// Prune old data from the Postgres database according to configured TTLs.
    ///
    /// Runs the same retention sweep that `brassclaw serve` performs
    /// automatically in the background (every 24 h), but as a one-shot
    /// foreground command.  Requires `BRASSCLAW_PG_URL` or `DATABASE_URL` to
    /// point at a running PostgreSQL instance.
    PruneOldData(PruneOldDataCommand),
}

impl MaintenanceCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        match self.subcommand {
            MaintenanceSubcommand::PruneOldData(cmd) => cmd.execute(context),
        }
    }
}

#[derive(Debug, Args)]
struct PruneOldDataCommand {}

impl PruneOldDataCommand {
    fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);

        println!("brassclaw maintenance prune-old-data: running retention sweep…");
        brassclaw_reborn_composition::retention_sweep::run_sweep(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("retention sweep failed: {e}"))?;
        println!("done.");

        Ok(())
    }
}

/// Build a Postgres pool from `BRASSCLAW_PG_URL` or `DATABASE_URL`.
async fn build_pg_pool() -> anyhow::Result<deadpool_postgres::Pool> {
    let url = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| {
            anyhow::anyhow!(
                "no PostgreSQL URL found; set BRASSCLAW_PG_URL or start \
                 `brassclaw serve` first to have embedded PostgreSQL started automatically"
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
    pool.get()
        .await
        .map_err(|e| anyhow::anyhow!("cannot connect to PostgreSQL: {e}"))?;
    Ok(pool)
}
