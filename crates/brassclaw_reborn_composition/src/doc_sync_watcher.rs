//! Phase P Step 9 — Doc-sync event wiring.
//!
//! Bridges two change-event sources to the `doc-sync` Action:
//!
//! (a) **File-watcher** — a [`notify`] watcher on `docs/agents-v3/*.md`.
//!     On any `.md` file event, upserts a one-shot `TriggerRecord` named
//!     `doc-sync::file-change` so the trigger poller fires the prompt
//!     `"run doc-sync"` and the orchestrator routes it to the `doc-sync`
//!     Action (intent examples match).
//!
//! (b) **Postgres NOTIFY listener** — a dedicated, non-pooled
//!     `tokio_postgres` connection that issues `LISTEN reborn_docus_changed`
//!     (installed by V081). On each notification, upserts a one-shot
//!     TriggerRecord named `doc-sync::docus-change`.
//!
//! # Deduplication
//!
//! Each event type uses a **fixed, well-known ULID** as the `trigger_id`.
//! Since `PostgresTriggerRepository::upsert_trigger` conflicts on
//! `(tenant_id, trigger_id) DO UPDATE`, a second rapid fire resets
//! `next_run_at` to now but does not create a second row.  The trigger
//! poller fires it once (due to `CompleteAfterFirstFire`) and removes it.
//!
//! # Design rationale
//!
//! The simplest viable dispatch path avoids building a new Action executor.
//! The watcher writes a synthetic trigger; the existing trigger poller picks
//! it up within its poll interval (≤ 60 s); the prompt `"run doc-sync"`
//! matches the doc-sync Action's intent examples; the orchestrator routes it
//! without an LLM call.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use brassclaw_host_api::{AgentId, SYSTEM_RESERVED_ID, TenantId, UserId};
use brassclaw_triggers::{
    TriggerCompletionPolicy, TriggerRecord, TriggerRepository, TriggerSchedule, TriggerSourceKind,
    TriggerState, TriggerId,
};
use notify::{Event, RecursiveMode, Watcher};
use tokio_postgres::NoTls;

/// Fixed ULID for the file-change trigger. Deterministic → idempotent upsert.
const FILE_CHANGE_TRIGGER_ULID: &str  = "00000000000000000000000001";
/// Fixed ULID for the docus-change trigger.
const DOCUS_CHANGE_TRIGGER_ULID: &str = "00000000000000000000000002";

/// Map a trigger name to its fixed ULID.  Unknown names fall back to a
/// freshly-generated random id (non-deduplicating but non-panicking).
fn trigger_id_for(name: &str) -> TriggerId {
    let ulid = match name {
        "doc-sync::file-change"  => FILE_CHANGE_TRIGGER_ULID,
        "doc-sync::docus-change" => DOCUS_CHANGE_TRIGGER_ULID,
        _ => return TriggerId::new(),
    };
    TriggerId::parse(ulid).unwrap_or_else(|_| TriggerId::new())
}

// ---------------------------------------------------------------------------
// DocSyncWatcher
// ---------------------------------------------------------------------------

/// Shared watcher context. Holds the trigger repository and the seeded scope
/// used to construct one-shot TriggerRecords.
pub(crate) struct DocSyncWatcher {
    trigger_repo: Arc<dyn TriggerRepository>,
    tenant_id: TenantId,
    system_user: UserId,
}

impl DocSyncWatcher {
    pub(crate) fn new(
        trigger_repo: Arc<dyn TriggerRepository>,
        tenant_id: TenantId,
    ) -> Self {
        Self {
            trigger_repo,
            tenant_id,
            system_user: UserId::from_trusted(SYSTEM_RESERVED_ID.to_string()),
        }
    }

    /// Upsert a one-shot TriggerRecord for `doc-sync`.
    ///
    /// Uses a deterministic `trigger_id` (UUID v5 of the name) so rapid
    /// repeated calls with the same `trigger_name` coalesce to one row via
    /// `ON CONFLICT (tenant_id, trigger_id) DO UPDATE`.
    ///
    /// The trigger fires within at most one poll interval (≤ 60 s).
    async fn fire_doc_sync(&self, trigger_name: &str) {
        let now = chrono::Utc::now();
        let schedule = match TriggerSchedule::cron("* * * * *") {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(
                    trigger_name,
                    error = %e,
                    "doc-sync watcher: failed to build cron schedule (non-fatal)"
                );
                return;
            }
        };
        let trigger_id = trigger_id_for(trigger_name);
        let record = TriggerRecord {
            trigger_id,
            tenant_id: self.tenant_id.clone(),
            creator_user_id: self.system_user.clone(),
            agent_id: Some(AgentId::from_trusted("default".to_string())),
            project_id: None,
            name: trigger_name.to_string(),
            source: TriggerSourceKind::Schedule,
            schedule,
            completion_policy: TriggerCompletionPolicy::CompleteAfterFirstFire,
            prompt: "run doc-sync".to_string(),
            state: TriggerState::Scheduled,
            next_run_at: now,
            last_run_at: None,
            last_fired_slot: None,
            last_status: None,
            active_fire_slot: None,
            active_run_ref: None,
            created_at: now,
        };
        if let Err(e) = self.trigger_repo.upsert_trigger(record).await {
            tracing::debug!(
                trigger_name,
                error = %e,
                "doc-sync watcher: upsert_trigger failed (non-fatal)"
            );
        } else {
            tracing::debug!(trigger_name, "doc-sync watcher: one-shot trigger upserted");
        }
    }
}

// ---------------------------------------------------------------------------
// (a) File-watcher task
// ---------------------------------------------------------------------------

/// Spawn a background task that watches `docs_dir` for `.md` file changes and
/// upserts a one-shot `doc-sync::file-change` trigger on each event.
///
/// Debounces events with a 500 ms quiet window.  Returns a `JoinHandle` that
/// resolves when `shutdown` is cancelled or a fatal watcher error occurs.
pub(crate) fn spawn_doc_sync_file_watcher(
    docs_dir: PathBuf,
    watcher_ctx: Arc<DocSyncWatcher>,
    shutdown: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Channel from notify's OS thread → tokio async task.
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<Result<Event, notify::Error>>(32);

        let tx_clone = tx.clone();
        let mut fs_watcher = match notify::RecommendedWatcher::new(
            move |event| {
                let _ = tx_clone.try_send(event);
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    "doc-sync file-watcher: failed to create watcher (non-fatal)"
                );
                return;
            }
        };

        if let Err(e) = fs_watcher.watch(&docs_dir, RecursiveMode::NonRecursive) {
            tracing::debug!(
                dir = %docs_dir.display(),
                error = %e,
                "doc-sync file-watcher: failed to watch dir (non-fatal)"
            );
            return;
        }

        tracing::debug!(
            dir = %docs_dir.display(),
            "doc-sync file-watcher: watching for .md changes"
        );

        // Debounce: 500 ms quiet window after the last event before firing.
        let debounce = Duration::from_millis(500);
        let mut pending = false;
        let mut deadline = tokio::time::Instant::now() + debounce;

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,

                maybe_event = rx.recv() => {
                    let Some(event) = maybe_event else { break };
                    let event = match event {
                        Ok(e) => e,
                        Err(e) => {
                            tracing::debug!(error = %e, "doc-sync file-watcher: event error");
                            continue;
                        }
                    };
                    if is_md_change_event(&event, &docs_dir) {
                        pending = true;
                        deadline = tokio::time::Instant::now() + debounce;
                    }
                }

                _ = tokio::time::sleep_until(deadline), if pending => {
                    pending = false;
                    watcher_ctx.fire_doc_sync("doc-sync::file-change").await;
                }
            }
        }

        drop(fs_watcher);
    })
}

/// Returns `true` if `event` is a write/create/remove of a `.md` file
/// directly inside `docs_dir` (not a sub-directory).
fn is_md_change_event(event: &Event, docs_dir: &Path) -> bool {
    use notify::EventKind::*;
    match &event.kind {
        Modify(_) | Create(_) | Remove(_) => {}
        _ => return false,
    }
    event.paths.iter().any(|p| {
        p.parent() == Some(docs_dir)
            && p.extension().and_then(|e| e.to_str()) == Some("md")
    })
}

// ---------------------------------------------------------------------------
// (b) Postgres NOTIFY listener task
// ---------------------------------------------------------------------------

/// Spawn a background task that `LISTEN`s on `reborn_docus_changed`
/// (installed by V081) and upserts a one-shot `doc-sync::docus-change`
/// trigger on each notification.
///
/// Opens a dedicated non-pooled `tokio_postgres` connection (LISTEN requires
/// a persistent connection — pooled connections lose their LISTEN state on
/// recycling).  Reconnects with exponential backoff on disconnection.
pub(crate) fn spawn_doc_sync_pg_listener(
    pg_url: String,
    watcher_ctx: Arc<DocSyncWatcher>,
    shutdown: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        const MAX_BACKOFF: Duration = Duration::from_secs(60);

        loop {
            if shutdown.is_cancelled() {
                break;
            }

            match pg_listener_session(&pg_url, watcher_ctx.clone(), shutdown.clone()).await {
                ListenerExit::Shutdown => break,
                ListenerExit::ConnectionError(e) => {
                    tracing::debug!(
                        error = %e,
                        backoff_secs = backoff.as_secs(),
                        "doc-sync pg-listener: disconnected, reconnecting"
                    );
                    tokio::select! {
                        _ = shutdown.cancelled() => break,
                        _ = tokio::time::sleep(backoff) => {}
                    }
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
        }
    })
}

enum ListenerExit {
    Shutdown,
    ConnectionError(String),
}

async fn pg_listener_session(
    pg_url: &str,
    watcher_ctx: Arc<DocSyncWatcher>,
    shutdown: tokio_util::sync::CancellationToken,
) -> ListenerExit {
    let (client, mut connection) = match tokio_postgres::connect(pg_url, NoTls).await {
        Ok(pair) => pair,
        Err(e) => return ListenerExit::ConnectionError(e.to_string()),
    };

    // Forward `AsyncMessage::Notification` events from the connection task to
    // the listener loop via a channel.
    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::channel::<()>(16);

    let conn_task = tokio::spawn(async move {
        use std::future::poll_fn;
        use std::task::Poll;
        poll_fn(|cx| {
            loop {
                match connection.poll_message(cx) {
                    Poll::Ready(Some(Ok(tokio_postgres::AsyncMessage::Notification(_)))) => {
                        // Best-effort: if the channel is full, the event is still
                        // coalesced (we care only about "something changed", not count).
                        let _ = notify_tx.try_send(());
                    }
                    Poll::Ready(Some(Ok(_))) => {} // notice or other; keep going
                    Poll::Ready(Some(Err(e))) => {
                        tracing::debug!(error = %e, "doc-sync pg-listener: conn msg error");
                        return Poll::Ready(());
                    }
                    Poll::Ready(None) => return Poll::Ready(()),
                    Poll::Pending => return Poll::Pending,
                }
            }
        })
        .await
    });

    if let Err(e) = client.batch_execute("LISTEN reborn_docus_changed").await {
        conn_task.abort();
        return ListenerExit::ConnectionError(e.to_string());
    }

    tracing::debug!("doc-sync pg-listener: LISTEN reborn_docus_changed registered");

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                conn_task.abort();
                return ListenerExit::Shutdown;
            }
            maybe = notify_rx.recv() => {
                match maybe {
                    Some(()) => {
                        tracing::debug!("doc-sync pg-listener: reborn_docus_changed");
                        watcher_ctx.fire_doc_sync("doc-sync::docus-change").await;
                    }
                    None => {
                        // Channel closed → connection task ended (error or PG shutdown).
                        return ListenerExit::ConnectionError(
                            "pg-listener connection task ended".to_string(),
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use brassclaw_host_api::TenantId;
    use brassclaw_triggers::{
        InMemoryTriggerRepository, TriggerCompletionPolicy, TriggerRepository, TriggerState,
    };

    use super::DocSyncWatcher;

    fn make_watcher() -> (Arc<InMemoryTriggerRepository>, Arc<DocSyncWatcher>) {
        let repo = Arc::new(InMemoryTriggerRepository::default());
        let watcher = Arc::new(DocSyncWatcher::new(
            repo.clone(),
            TenantId::new("test-tenant").expect("tenant"),
        ));
        (repo, watcher)
    }

    #[tokio::test]
    async fn fire_doc_sync_inserts_scheduled_one_shot_trigger() {
        let (repo, watcher) = make_watcher();
        watcher.fire_doc_sync("doc-sync::file-change").await;

        let tenant = TenantId::new("test-tenant").expect("tenant");
        let triggers = repo.list_triggers(tenant).await.expect("list");
        assert_eq!(triggers.len(), 1, "expected exactly one trigger");
        let t = &triggers[0];
        assert_eq!(t.name, "doc-sync::file-change");
        assert_eq!(t.prompt, "run doc-sync");
        assert_eq!(t.state, TriggerState::Scheduled);
        assert_eq!(t.completion_policy, TriggerCompletionPolicy::CompleteAfterFirstFire);
    }

    #[tokio::test]
    async fn fire_doc_sync_second_call_coalesces_to_one_row() {
        // The second call uses the same deterministic trigger_id → `upsert_trigger`
        // on InMemoryTriggerRepository replaces the existing record
        // (HashMap insert overwrites). Only one row must exist after two calls.
        let (repo, watcher) = make_watcher();
        watcher.fire_doc_sync("doc-sync::file-change").await;
        watcher.fire_doc_sync("doc-sync::file-change").await;

        let tenant = TenantId::new("test-tenant").expect("tenant");
        let triggers = repo.list_triggers(tenant).await.expect("list");
        let file_change_count = triggers
            .iter()
            .filter(|t| t.name == "doc-sync::file-change")
            .count();
        assert_eq!(
            file_change_count, 1,
            "deterministic trigger_id must coalesce duplicate events to one row"
        );
    }

    #[tokio::test]
    async fn fire_doc_sync_different_names_produce_separate_rows() {
        let (repo, watcher) = make_watcher();
        watcher.fire_doc_sync("doc-sync::file-change").await;
        watcher.fire_doc_sync("doc-sync::docus-change").await;

        let tenant = TenantId::new("test-tenant").expect("tenant");
        let triggers = repo.list_triggers(tenant).await.expect("list");
        assert_eq!(triggers.len(), 2, "two distinct names → two rows");
    }
}
