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

    /// Backfill vector embeddings for existing chat-memory records.
    ///
    /// Iterates `brassclaw_memory_chat_records` rows whose `source_ref` is NULL
    /// or whose chunk subtree has no embedding vector, chunks and embeds each
    /// record's content, and writes chunk rows under the VFS.  Idempotent: safe
    /// to interrupt and resume.
    ///
    /// Requires an `embedding`-role provider to be configured via
    /// `brassclaw config set embedding.provider_id <id>`.  If no embedding
    /// provider is active, the command exits successfully with no work done.
    ///
    /// Requires `BRASSCLAW_PG_URL` or `DATABASE_URL` to point at a running
    /// PostgreSQL instance.
    BackfillEmbeddings(BackfillEmbeddingsCommand),
}

impl MaintenanceCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        match self.subcommand {
            MaintenanceSubcommand::PruneOldData(cmd) => cmd.execute(context),
            MaintenanceSubcommand::BackfillEmbeddings(cmd) => cmd.execute(context),
        }
    }
}

// ---------------------------------------------------------------------------
// prune-old-data
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// backfill-embeddings
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
struct BackfillEmbeddingsCommand {
    /// Tenant ID to backfill (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,

    /// Number of records to process per batch (default: 100).
    #[arg(long, default_value_t = 100i64)]
    batch_size: i64,
}

impl BackfillEmbeddingsCommand {
    fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    async fn run_async(self) -> anyhow::Result<()> {
        let pool = build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);

        println!(
            "brassclaw maintenance backfill-embeddings: tenant={}, batch_size={}",
            self.tenant, self.batch_size
        );

        let result = brassclaw_reborn_composition::retention_sweep::run_backfill_embeddings(
            pool,
            &self.tenant,
            self.batch_size,
        )
        .await
        .map_err(|e| anyhow::anyhow!("backfill-embeddings failed: {e}"))?;

        println!(
            "done — indexed: {}, failed: {}",
            result.indexed, result.failed
        );
        Ok(())
    }

    #[cfg(not(all(feature = "postgres", feature = "root-llm-provider")))]
    async fn run_async(self) -> anyhow::Result<()> {
        anyhow::bail!(
            "backfill-embeddings requires the `postgres` and `root-llm-provider` features"
        )
    }
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

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
