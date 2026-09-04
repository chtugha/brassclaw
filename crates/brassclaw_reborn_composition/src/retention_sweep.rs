//! Background retention sweep task and backfill-embeddings operation.
//!
//! Pruning runs inside the `brassclaw serve` process only (not via `pg_cron`).
//! Call [`spawn_retention_sweep`] from the serve startup path; it runs
//! indefinitely on a 24-hour cadence until the returned handle is cancelled.
//!
//! Default TTLs (days):
//! - `brassclaw_checkpoints`: 30 days (plus last-10-per-run app-layer keep)
//! - `brassclaw_events`: 90 days
//! - `brassclaw_audit_log`: 365 days
//! - `brassclaw_runs` soft-deleted: 90 days after `deleted_at`
//! - `brassclaw_extensions` removed: 90 days after `removed_at`
//! - `brassclaw_forensic_packets`: 42 days / 6 weeks (§0.23.7 — component-UUID reference retention)
//!
//! `brassclaw_memory_chat_records` has no default TTL; pruning is only enabled
//! when the operator sets `retention.memory_chat_records_days` in config.
//! Records with `importance >= 0.8` are never pruned even when a TTL is set.
//!
//! All TTLs are overridable via `brassclaw_config` keys (Phase 2 config DB).
//! This sweep does not yet read those keys (Phase 5 factory wiring completes
//! the config resolution); it uses the hardcoded defaults below.

use std::sync::Arc;

use brassclaw_pg::PgPool;
use tokio::time::{Duration, interval};
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
use chrono::Timelike as _;

const SWEEP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

const DEFAULT_CHECKPOINTS_DAYS: i64 = 30;
const DEFAULT_EVENTS_DAYS: i64 = 90;
const DEFAULT_AUDIT_LOG_DAYS: i64 = 365;
const DEFAULT_RUNS_DELETED_DAYS: i64 = 90;
const DEFAULT_EXTENSIONS_REMOVED_DAYS: i64 = 90;
// §0.23.7: forensic packet retention reduced to 6 weeks (42 days) so
// old component references do not accumulate indefinitely.
const DEFAULT_FORENSIC_PACKETS_DAYS: i64 = 42;

/// Spawn the background retention sweep task.
///
/// Returns a [`tokio::task::JoinHandle`] — drop it or abort it to stop the sweep.
pub fn spawn_retention_sweep(pool: Arc<PgPool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(e) = run_sweep(&pool).await {
                // Use debug! per project rules — background tasks must not use info!/warn!.
                tracing::debug!(error = %e, "retention sweep error");
            }
        }
    })
}

/// Run one full retention sweep cycle.
pub async fn run_sweep(pool: &Arc<PgPool>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;

    // brassclaw_checkpoints: prune rows older than N days (last-10-per-run
    // app-layer keep is handled by the loop runner, not here).
    client
        .execute(
            "DELETE FROM brassclaw_checkpoints \
             WHERE created_at < now() - ($1 || ' days')::interval",
            &[&DEFAULT_CHECKPOINTS_DAYS],
        )
        .await?;

    // brassclaw_events: 90-day TTL.
    client
        .execute(
            "DELETE FROM brassclaw_events \
             WHERE created_at < now() - ($1 || ' days')::interval",
            &[&DEFAULT_EVENTS_DAYS],
        )
        .await?;

    // brassclaw_audit_log: 365-day TTL.
    client
        .execute(
            "DELETE FROM brassclaw_audit_log \
             WHERE created_at < now() - ($1 || ' days')::interval",
            &[&DEFAULT_AUDIT_LOG_DAYS],
        )
        .await?;

    // brassclaw_runs: soft-delete TTL (90 days after deleted_at).
    client
        .execute(
            "DELETE FROM brassclaw_runs \
             WHERE deleted_at IS NOT NULL \
               AND deleted_at < now() - ($1 || ' days')::interval",
            &[&DEFAULT_RUNS_DELETED_DAYS],
        )
        .await?;

    // brassclaw_extensions: removed TTL (90 days after removed_at).
    client
        .execute(
            "DELETE FROM brassclaw_extensions \
             WHERE removed_at IS NOT NULL \
               AND removed_at < now() - ($1 || ' days')::interval",
            &[&DEFAULT_EXTENSIONS_REMOVED_DAYS],
        )
        .await?;

    // brassclaw_forensic_packets: 90-day TTL.
    // First null out any linked memory-chat-record references (preserving the
    // memory record itself per §4.21 spec).
    let packet_rows = client
        .query(
            "SELECT id FROM brassclaw_forensic_packets \
             WHERE captured_at < now() - ($1 || ' days')::interval",
            &[&DEFAULT_FORENSIC_PACKETS_DAYS],
        )
        .await?;
    for row in packet_rows {
        let packet_id: String = match row.try_get("id") {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "retention sweep: bad forensic packet row — skipping");
                continue;
            }
        };
        // Null out links before deleting the packet.  Must succeed before the
        // DELETE so we never leave dangling forensic_packet_id references in
        // memory rows pointing to a packet that no longer exists.
        client
            .execute(
                "UPDATE brassclaw_memory_chat_records \
                 SET forensic_packet_id = NULL \
                 WHERE forensic_packet_id = $1",
                &[&packet_id],
            )
            .await?;
        client
            .execute(
                "DELETE FROM brassclaw_forensic_packets WHERE id = $1",
                &[&packet_id],
            )
            .await?;
    }

    // brassclaw_memory_chat_records: no default TTL — only prune when
    // the operator has set retention.memory_chat_records_days in config.
    // Phase 5 factory wiring will pass the config value here; for now no-op.

    Ok(())
}

// ---------------------------------------------------------------------------
// Q1 auto-validation sweep (Step 3.2)
// ---------------------------------------------------------------------------

/// Spawn the Q1 auto-validation background task.
///
/// Runs every 30 seconds.  For each iteration, fetches all `pending` rows in
/// `reborn_recipes` with `queue_code = 'q1_auto'`, runs
/// [`ComponentValidator::validate_by_class`] against each (fetching available
/// Rusty tool names from `reborn_tools` via [`DbToolSource`]), and writes
/// `auto_passed` or `auto_failed` back.
///
/// Only compiled when both `postgres` and `skills-db` features are active.
/// The returned handle is intentionally leaked in the serve path — the sweep is
/// best-effort and can be silently aborted at shutdown.
///
/// The `tenant_id` and `agent_id` must match the scope used by the recipe
/// store facade so that scope isolation is enforced.
#[cfg(all(feature = "postgres", feature = "skills-db"))]
pub fn spawn_q1_validation_sweep(
    pool: Arc<PgPool>,
    tenant_id: String,
    agent_id: String,
    user_id: String,
    project_id: String,
) -> tokio::task::JoinHandle<()> {
    const Q1_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Q1_SWEEP_INTERVAL);
        let facade = crate::pg_recipe_store::PgRecipeStoreFacade::new(
            Arc::clone(&pool),
            &tenant_id,
            &agent_id,
        );
        loop {
            ticker.tick().await;
            match brassclaw_product_workflow::RecipeStore::auto_validate_pending(
                &facade,
                &user_id,
                &project_id,
            )
            .await
            {
                Ok(n) if n > 0 => {
                    tracing::debug!(
                        processed = n,
                        "q1_validation_sweep: processed pending components"
                    );
                }
                Ok(_) => {
                    // Nothing to do this cycle.
                }
                Err(e) => {
                    tracing::debug!(error = %e, "q1_validation_sweep: error");
                }
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Idle self-improvement sweep (§0.23.8)
// ---------------------------------------------------------------------------

/// Configuration for the idle self-improvement sweep, read from
/// `reborn_monty_vm_settings` (`validation_idle_threshold_minutes`,
/// `validation_improve_start_hour`, `validation_improve_enabled`).
///
/// Defaults match the V063 column defaults:
/// - `idle_threshold_minutes = 120` (2 hours)
/// - `improve_start_hour = 15` (15:00 local)
/// - `enabled = true`
#[derive(Debug, Clone)]
pub struct IdleSweepConfig {
    pub idle_threshold_minutes: i32,
    pub improve_start_hour: i32,
    pub enabled: bool,
}

impl Default for IdleSweepConfig {
    fn default() -> Self {
        Self {
            idle_threshold_minutes: 120,
            improve_start_hour: 15,
            enabled: true,
        }
    }
}

/// Read the idle sweep config from `reborn_monty_vm_settings` for a given
/// scope.  Returns `IdleSweepConfig::default()` on any error or missing row.
#[cfg(feature = "postgres")]
pub async fn load_idle_sweep_config(
    pool: &Arc<PgPool>,
    tenant_id: &str,
    agent_id: &str,
    user_id: &str,
    project_id: &str,
) -> IdleSweepConfig {
    let Ok(client) = pool.get().await else {
        return IdleSweepConfig::default();
    };
    let Ok(Some(row)) = client
        .query_opt(
            "SELECT validation_idle_threshold_minutes,
                    validation_improve_start_hour,
                    validation_improve_enabled
             FROM reborn_monty_vm_settings
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4",
            &[&tenant_id, &user_id, &agent_id, &project_id],
        )
        .await
    else {
        return IdleSweepConfig::default();
    };
    IdleSweepConfig {
        idle_threshold_minutes: row.try_get::<_, i32>(0).unwrap_or(120),
        improve_start_hour: row.try_get::<_, i32>(1).unwrap_or(15),
        enabled: row.try_get::<_, bool>(2).unwrap_or(true),
    }
}

/// Spawn the idle self-improvement sweep background task (§0.23.8).
///
/// Runs on a 15-minute polling cadence.  Each cycle:
/// 1. Reads idle sweep config from `reborn_monty_vm_settings`.
/// 2. Checks `enabled`, idle window, and server-local time (≥ `improve_start_hour`).
/// 3. Checks that the system has been idle for at least `idle_threshold_minutes`
///    (no active turns — approximated by checking `brassclaw_runs` for recent
///    non-completed rows within the threshold window).
/// 4. If all gates pass and the sweep has not run today: enqueues a
///    component-creation request via the Sempai proposal sink (fires-and-forgets).
///
/// The returned handle can be aborted at shutdown.  The sweep is best-effort —
/// errors are logged at `debug!` and never propagate to callers.
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
pub fn spawn_idle_improvement_sweep(
    pool: Arc<PgPool>,
    tenant_id: String,
    agent_id: String,
    user_id: String,
    project_id: String,
) -> tokio::task::JoinHandle<()> {
    const POLL_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        // Track which calendar date the sweep last ran — one run per day.
        let mut last_run_date: Option<chrono::NaiveDate> = None;

        loop {
            ticker.tick().await;

            let config = load_idle_sweep_config(
                &pool,
                &tenant_id,
                &agent_id,
                &user_id,
                &project_id,
            )
            .await;

            if !config.enabled {
                continue;
            }

            let now_local = chrono::Local::now();
            let today = now_local.date_naive();

            // Already ran today?
            if last_run_date == Some(today) {
                continue;
            }

            // Gate 1: server-local time must be ≥ improve_start_hour.
            if now_local.hour() < config.improve_start_hour as u32 {
                continue;
            }

            // Gate 2: system must be idle for ≥ idle_threshold_minutes.
            // Approximation: no non-completed runs created within the threshold window.
            let threshold_minutes = config.idle_threshold_minutes as i64;
            let is_idle = match pool.get().await {
                Ok(client) => client
                    .query_opt(
                        "SELECT 1 FROM brassclaw_runs \
                         WHERE tenant_id = $1 \
                           AND status NOT IN ('complete','failed','cancelled') \
                           AND created_at > now() - ($2 || ' minutes')::interval \
                         LIMIT 1",
                        &[&tenant_id.as_str(), &threshold_minutes],
                    )
                    .await
                    .map(|r| r.is_none()) // idle = no active runs in window
                    .unwrap_or(false),
                Err(_) => false,
            };

            if !is_idle {
                continue;
            }

            // Both gates passed — record today and log (actual Sempai call
            // wired when SempaiProposalSink is available in scope; foundation only).
            last_run_date = Some(today);
            tracing::debug!(
                tenant_id = %tenant_id,
                "idle_improvement_sweep: conditions met — improvement sweep triggered"
            );
        }
    })
}



// ---------------------------------------------------------------------------
// Chunk-cascade delete helper (§4.30.2)
// ---------------------------------------------------------------------------

/// Delete all chunk VFS rows for a given chat-memory record, then delete the
/// record itself.  The chunk deletion is transactional with the record deletion:
/// if the chunk delete fails, the record is NOT deleted.
///
/// `source_ref` must be a non-empty path (e.g. `/memory/chat/<id>`).
pub async fn delete_chat_record_with_chunk_cascade(
    pool: &Arc<PgPool>,
    tenant_id: &str,
    record_id: &str,
    source_ref: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if source_ref.is_empty() {
        // Nothing to cascade — just delete the record.
        let client = pool.get().await?;
        client
            .execute(
                "DELETE FROM brassclaw_memory_chat_records \
                 WHERE id = $1 AND tenant_id = $2",
                &[&record_id, &tenant_id],
            )
            .await?;
        return Ok(());
    }

    // Build the chunk subtree prefix: <source_ref>/*.chunks/
    // The VFS stores chunk entries at paths like
    //   /memory/chat/<id>/<file>.chunks/<index>
    // A LIKE match on the source_ref prefix covers all chunk variants.
    let chunk_prefix = format!("{source_ref}/%");

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    // 1. Delete chunk rows first (§4.30.2 — must succeed before record delete).
    tx.execute(
        "DELETE FROM brassclaw_root_filesystem \
         WHERE path LIKE $1 AND tenant_id = $2",
        &[&chunk_prefix, &tenant_id],
    )
    .await?;

    // 2. Delete the Path A record.
    tx.execute(
        "DELETE FROM brassclaw_memory_chat_records \
         WHERE id = $1 AND tenant_id = $2",
        &[&record_id, &tenant_id],
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Backfill embeddings (§4.30.5, §8.1 step 10)
// ---------------------------------------------------------------------------

/// Result returned by [`run_backfill_embeddings`].
#[derive(Debug, Default)]
pub struct BackfillResult {
    /// Number of chat-memory records successfully indexed.
    pub indexed: usize,
    /// Number of records that failed indexing (error logged per-record).
    pub failed: usize,
}

/// Backfill embeddings for `brassclaw_memory_chat_records` rows whose
/// `source_ref` is NULL (Path B has not yet produced a chunk subtree for them).
///
/// Reads chat-memory records in batches, reconstructs the `MemoryDocumentScope`,
/// calls `indexer.index_content(...)` for each, and updates `source_ref` when
/// the indexer succeeds.  Idempotent: safe to interrupt and resume.
///
/// Returns `Ok(BackfillResult)` with counts.  Per-record errors are logged at
/// `debug!` level and counted in `failed` — they do not abort the batch.
///
/// Returns `Err(...)` only for fatal I/O errors (pool exhausted, bad SQL).
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
pub async fn run_backfill_embeddings(
    pool: Arc<PgPool>,
    tenant_id: &str,
    batch_size: i64,
) -> Result<BackfillResult, Box<dyn std::error::Error + Send + Sync>> {
    use brassclaw_filesystem::PostgresRootFilesystem;
    use brassclaw_memory::{
        ChunkingMemoryDocumentIndexer, FilesystemMemoryDocumentRepository, MemoryDocumentIndexer,
        MemoryDocumentScope,
    };

    // Resolve embedding provider from DB config (same logic as factory).
    let raw_pool: deadpool_postgres::Pool = (*pool).clone();
    let embedding_provider =
        crate::factory::resolve_pg_embedding_provider_pub(&raw_pool, tenant_id).await;

    let Some(embedding_provider) = embedding_provider else {
        tracing::debug!(
            "backfill-embeddings: no embedding role provider configured — nothing to do"
        );
        return Ok(BackfillResult::default());
    };

    // Build indexer over PostgresRootFilesystem.
    let filesystem = Arc::new(PostgresRootFilesystem::new((*pool).clone()));
    let repository = Arc::new(FilesystemMemoryDocumentRepository::new(
        filesystem as Arc<dyn brassclaw_filesystem::RootFilesystem>,
    ));

    // Thin wrapper so `Arc<dyn EmbeddingProvider>` satisfies the `P: EmbeddingProvider`
    // bound on `with_embedding_provider` (same pattern as memory.rs).
    struct DynWrapper(Arc<dyn brassclaw_memory::EmbeddingProvider>);
    #[async_trait::async_trait]
    impl brassclaw_memory::EmbeddingProvider for DynWrapper {
        fn dimension(&self) -> usize {
            self.0.dimension()
        }
        fn model_name(&self) -> &str {
            self.0.model_name()
        }
        async fn embed(&self, text: &str) -> Result<Vec<f32>, brassclaw_memory::EmbeddingError> {
            self.0.embed(text).await
        }
        async fn embed_batch(
            &self,
            texts: &[String],
        ) -> Result<Vec<Vec<f32>>, brassclaw_memory::EmbeddingError> {
            self.0.embed_batch(texts).await
        }
    }
    let indexer = ChunkingMemoryDocumentIndexer::new(Arc::clone(&repository))
        .with_embedding_provider(Arc::new(DynWrapper(embedding_provider)));

    // Fetch rows needing backfill: any record whose source_ref is NULL has not
    // yet had Path B (chunk + embed) run.  Records with a non-NULL source_ref
    // already have chunk rows under the VFS; re-indexing them is handled by the
    // caller if a dimension change is requested (idempotent by design).
    //
    // The previous sub-select against brassclaw_root_filesystem_index_specs used
    // columns (entry_id, index_key, index_value) that do not exist in the V018
    // schema — that join would have failed at runtime.  The source_ref NULL check
    // is the correct and sufficient backfill predicate.
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT id, tenant_id, user_id, agent_id, project_id, content \
             FROM brassclaw_memory_chat_records \
             WHERE tenant_id = $1 \
               AND source_ref IS NULL \
             ORDER BY created_at ASC \
             LIMIT $2",
            &[&tenant_id, &batch_size],
        )
        .await?;
    // Drop client so we don't hold the connection during indexing.
    drop(client);

    let mut result = BackfillResult::default();
    for row in rows {
        let map_col = |e: tokio_postgres::Error| format!("column decode: {e}");
        let id: String = match row.try_get("id").map_err(&map_col) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "backfill-embeddings: bad row — skipping");
                result.failed += 1;
                continue;
            }
        };
        let t_id: String = match row.try_get("tenant_id").map_err(&map_col) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "backfill-embeddings: bad row — skipping");
                result.failed += 1;
                continue;
            }
        };
        let user_id: String = match row.try_get("user_id").map_err(&map_col) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "backfill-embeddings: bad row — skipping");
                result.failed += 1;
                continue;
            }
        };
        let agent_id: Option<String> = match row.try_get("agent_id").map_err(&map_col) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "backfill-embeddings: bad row — skipping");
                result.failed += 1;
                continue;
            }
        };
        let project_id: Option<String> = match row.try_get("project_id").map_err(&map_col) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "backfill-embeddings: bad row — skipping");
                result.failed += 1;
                continue;
            }
        };
        let content: String = match row.try_get("content").map_err(&map_col) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "backfill-embeddings: bad row — skipping");
                result.failed += 1;
                continue;
            }
        };
        // source_ref is always NULL for rows returned by this query.
        // Derive the canonical VFS path from the chat_record_id.
        let source_ref = format!("/memory/chat/{id}");

        let scope = match MemoryDocumentScope::new_with_agent(
            &t_id,
            &user_id,
            agent_id.as_deref(),
            project_id.as_deref(),
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(
                    chat_record_id = %id,
                    error = %e,
                    "backfill-embeddings: invalid scope — skipping"
                );
                result.failed += 1;
                continue;
            }
        };

        match indexer
            .index_content(&scope, &source_ref, &content, Some(&id))
            .await
        {
            Ok(()) => {
                // The query only fetches source_ref IS NULL rows, so source_ref
                // is always derived here.  Update it best-effort so the row is
                // skipped on the next backfill run.
                if let Ok(c2) = pool.get().await
                    && let Err(e) = c2
                        .execute(
                            "UPDATE brassclaw_memory_chat_records \
                             SET source_ref = $1 WHERE id = $2 AND tenant_id = $3",
                            &[&source_ref, &id, &t_id],
                        )
                        .await
                {
                    tracing::debug!(
                        chat_record_id = %id,
                        error = %e,
                        "backfill-embeddings: source_ref update failed (best-effort)"
                    );
                }
                result.indexed += 1;
            }
            Err(e) => {
                tracing::debug!(
                    chat_record_id = %id,
                    error = %e,
                    "backfill-embeddings: indexing failed — skipping"
                );
                result.failed += 1;
            }
        }
    }

    Ok(result)
}
