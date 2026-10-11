//! Owned PostgreSQL lifetime for one DB-touching management command.
//!
//! Callers run migrations and their operation inside `with_postgres`. Every
//! normal success/error path closes the pool before graceful embedded shutdown.
//! A reused server or external URL is never stopped by this command. Operations
//! must finish their database work inside the callback, returning owned data,
//! without spawning detached database users or returning pooled clients.

use std::{future::Future, sync::Arc};

use brassclaw_embedded_postgres::ManagedPostgres;

use crate::context::RebornCliContext;

pub(crate) async fn with_postgres<T, F, Fut>(
    context: &RebornCliContext,
    operation: F,
) -> anyhow::Result<T>
where
    F: FnOnce(Arc<deadpool_postgres::Pool>) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let database = CommandPostgres::connect(context).await?;
    let result = operation(Arc::clone(&database.pool)).await;
    database.finish(result).await
}

struct CommandPostgres {
    pool: Arc<deadpool_postgres::Pool>,
    embedded: Option<ManagedPostgres>,
}

impl CommandPostgres {
    async fn connect(context: &RebornCliContext) -> anyhow::Result<Self> {
        // Explicit endpoints never acquire ownership of an embedded server.
        if let Ok(url) =
            std::env::var("BRASSCLAW_PG_URL").or_else(|_| std::env::var("DATABASE_URL"))
        {
            return Ok(Self {
                pool: Arc::new(connect_url(&url).await?),
                embedded: None,
            });
        }

        let config = brassclaw_embedded_postgres::EmbeddedPostgresConfig::from_reborn_home(
            context.boot_config().home().path(),
        );
        let managed = ManagedPostgres::start(config)
            .await
            .map_err(|e| anyhow::anyhow!("failed to start/connect to embedded Postgres: {e}"))?;
        match connect_url(&managed.connection_url()).await {
            Ok(pool) => Ok(Self {
                pool: Arc::new(pool),
                embedded: Some(managed),
            }),
            Err(error) => combine_result(Err(error), managed.shutdown().await.map_err(Into::into)),
        }
    }

    async fn finish<T>(&self, result: anyhow::Result<T>) -> anyhow::Result<T> {
        self.pool.close();
        let shutdown = match &self.embedded {
            Some(managed) => managed.shutdown().await.map_err(Into::into),
            None => Ok(()),
        };
        combine_result(result, shutdown)
    }
}

impl Drop for CommandPostgres {
    fn drop(&mut self) {
        // Also close on panic/cancellation before ManagedPostgres's visible
        // emergency fallback. Normal returns use the explicit async finish.
        self.pool.close();
    }
}

fn combine_result<T>(result: anyhow::Result<T>, shutdown: anyhow::Result<()>) -> anyhow::Result<T> {
    match (result, shutdown) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error.context(
            "database operation completed, but PostgreSQL shutdown failed; do not repeat the operation solely to retry shutdown",
        )),
        (Err(error), Err(shutdown)) => Err(error.context(format!(
            "PostgreSQL shutdown also failed: {shutdown:#}"
        ))),
    }
}

async fn connect_url(url: &str) -> anyhow::Result<deadpool_postgres::Pool> {
    let pool = brassclaw_pg::pool::build_pool_with_max_size(url, 4)?;
    if let Err(error) = pool.get().await {
        pool.close();
        anyhow::bail!("cannot connect to PostgreSQL: {error}");
    }
    Ok(pool)
}
