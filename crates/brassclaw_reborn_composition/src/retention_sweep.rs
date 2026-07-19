//! Background retention sweep task.
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
//! - `brassclaw_forensic_packets`: 90 days
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

const SWEEP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

const DEFAULT_CHECKPOINTS_DAYS: i64 = 30;
const DEFAULT_EVENTS_DAYS: i64 = 90;
const DEFAULT_AUDIT_LOG_DAYS: i64 = 365;
const DEFAULT_RUNS_DELETED_DAYS: i64 = 90;
const DEFAULT_EXTENSIONS_REMOVED_DAYS: i64 = 90;
const DEFAULT_FORENSIC_PACKETS_DAYS: i64 = 90;

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
        let packet_id: String = row.get(0);
        // Null out links before deleting the packet.
        client
            .execute(
                "UPDATE brassclaw_memory_chat_records \
                 SET forensic_packet_id = NULL \
                 WHERE forensic_packet_id = $1",
                &[&packet_id],
            )
            .await
            .unwrap_or_default();
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
