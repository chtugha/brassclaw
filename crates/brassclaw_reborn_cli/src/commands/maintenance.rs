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
    /// (Path B has not yet run for those records), chunks and embeds each
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
    fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            // Step 2: run schema migrations (idempotent) before any DB operation.
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            println!("brassclaw maintenance prune-old-data: running retention sweep…");
            brassclaw_reborn_composition::retention_sweep::run_sweep(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("retention sweep failed: {e}"))?;
            println!("done.");

            Ok(())
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// backfill-embeddings
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
struct BackfillEmbeddingsCommand {
    /// Storage namespace to backfill; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,

    /// Number of records to process per batch (default: 100).
    #[arg(long, default_value_t = 100i64)]
    batch_size: i64,
}

impl BackfillEmbeddingsCommand {
    fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        // Match serve's installation identity, independently of the data tenant
        // passed to backfill. default_owner is an actor, not the secret-store owner.
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        let secret_store_tenant = context.storage_namespace(None)?;
        brassclaw_reborn_composition::host_api::TenantId::new(&secret_store_tenant)
            .map_err(|error| anyhow::anyhow!("invalid runtime secret-store owner: {error}"))?;
        let runtime_storage_root = context.boot_config().home().path().join("db");
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            // Step 2: run schema migrations (idempotent) before any DB operation.
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            println!(
                "brassclaw maintenance backfill-embeddings: tenant={}, batch_size={}",
                tenant, self.batch_size
            );

            let result = brassclaw_reborn_composition::retention_sweep::run_backfill_embeddings(
                pool,
                &tenant,
                self.batch_size,
                &runtime_storage_root,
                &secret_store_tenant,
            )
            .await
            .map_err(|e| anyhow::anyhow!("backfill-embeddings failed: {e}"))?;

            println!(
                "done — indexed: {}, failed: {}",
                result.indexed, result.failed
            );
            Ok(())
        })
        .await
    }

    #[cfg(not(all(feature = "postgres", feature = "root-llm-provider")))]
    async fn run_async(self, _context: RebornCliContext) -> anyhow::Result<()> {
        anyhow::bail!(
            "backfill-embeddings requires the `postgres` and `root-llm-provider` features"
        )
    }
}
