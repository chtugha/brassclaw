//! Cross-turn-persistent Monty turn driver (C.6 slice 4c/4d).
//!
//! [`PersistentMontyDriver`] is the composition-layer impl of the turns-native
//! [`MontyTurnDriverPort`] (slice 4b). It owns the conversation-keyed
//! [`MontySessionRegistry`] (slice 3) + the engine dependencies needed to load
//! the live [`Thread`], build or resume a parked [`MontySession`], and drive it
//! to a yield. `TurnRunnerWorker` (slice 4d) holds an `Arc<dyn
//! MontyTurnDriverPort>` and calls [`PersistentMontyDriver::drive_turn`]
//! directly for every Monty turn, bypassing `driver_registry` / the canonical
//! stage pipeline (C6-1=B).
//!
//! ## Turn flow (prime-then-resume — C6-2=B true VM persistence)
//!
//! `basic_mode.py` parks at `host.await_next_turn()` each turn instead of
//! returning. The first `await_next_turn()` on a fresh VM has nothing to resume
//! from, so turn 1 needs TWO drives: a **prime** (`drive_to_yield(None)`) that
//! runs `_seed_history` + `check_signals` and parks at the first
//! `await_next_turn()`, then a **resume** (`drive_to_yield(Some(turn1_input))`)
//! that feeds turn 1's input, processes it, and parks again. Turn 2+ reuses the
//! parked session with a single resume drive.
//!
//! ## Effect executor (per-run)
//!
//! The [`TierZeroEffectExecutorBuilder`] held by the driver builds a
//! production [`EffectExecutor`] at the start of each [`drive_turn`] call (one
//! per turn, not one per driver lifetime) by snapshotting the extension surface
//! and resolving grants for the turn's run context. This is the same
//! build-per-run pattern used by `PgOrchestratorLookup::run_tier_zero`.
//!
//! ## Signals (user-locked A — signal broker)
//!
//! The driver owns a [`SignalBroker`] holding the per-conversation
//! `SignalSender` for the turn currently in flight. `drive_turn` creates a
//! fresh signal channel per turn, registers the sender in the broker (so the
//! turn runner — slice 4d — can forward `Stop`/`Suspend`/`InjectMessage`), and
//! feeds the receiver to `drive_to_yield` so `host.check_signals` sees real
//! in-turn signals. The broker entry is cleared when the turn ends.
//!
//! ## Completion handshake
//!
//! The orchestrator posts its reply via `host.post_reply` (persisted outside
//! the `LoopExit` ref mechanism), so the driver returns a minimal
//! [`LoopExit::Completed`] with [`LoopCompletionKind::NoReply`] + empty refs.
//! The trusted applier accepts empty-ref completed exits (the orchestrator owns
//! the durable reply artifact). C6-4=C: full e2e "drives a turn" verification is
//! CI/Docker; local verification = unit tests for the signal broker, the
//! exit-id construction, and a `Send + Sync` assert on the driver.
//!
//! ## User-input source
//!
//! The current turn's user input is loaded from the `SessionThreadService` via
//! `latest_thread_message` (last finalized `User` message). The engine `Thread`
//! returned by `PgThreadEngineStore::load_thread` carries an empty `messages`
//! vec — it only maps metadata — so reading from `thread.messages` would always
//! produce `""`, causing the VM to take the empty-input `FINAL(...)` path on
//! every turn.

#![forbid(unsafe_code)]

#[cfg(feature = "skills-db")]
use std::collections::HashMap;
#[cfg(feature = "skills-db")]
use std::sync::Arc;

#[cfg(feature = "skills-db")]
use async_trait::async_trait;
#[cfg(feature = "skills-db")]
use monty::MontyObject;
#[cfg(feature = "skills-db")]
use tokio::sync::Mutex;
#[cfg(feature = "skills-db")]
use tracing::debug;

#[cfg(feature = "skills-db")]
use crate::runtime::TierZeroEffectExecutorBuilder;
#[cfg(feature = "skills-db")]
use crate::session_registry::MontySessionRegistry;
#[cfg(feature = "skills-db")]
use brassclaw_engine::{
    Store,
    capability::{lease::LeaseManager, policy::PolicyEngine},
    executor::{
        ComponentPort, DynamicToolPort, KohaiPort,
        orchestrator::{MontySession, OrchestratorYield, prepare_monty_session},
    },
    gate::GateController,
    runtime::messaging::{SignalReceiver, SignalSender, ThreadSignal, signal_channel},
    traits::effect::EffectExecutor,
    types::{
        event::ThreadEvent,
        message::MessageRole,
        thread::{Thread, ThreadId as EngineThreadId},
    },
};
#[cfg(feature = "skills-db")]
use brassclaw_threads::{
    AppendAssistantDraftRequest, LatestThreadMessageRequest, MessageContent, MessageKind,
    MessageStatus, SessionThreadService, ThreadScope,
};
#[cfg(feature = "skills-db")]
use brassclaw_turns::{
    LoopCompleted, LoopCompletionKind, LoopExit, LoopExitId, TurnRunId, TurnScope,
    run_profile::{
        AgentLoopDriverError, AgentLoopDriverHost, AgentLoopDriverRunRequest, LoopRunContext,
        MontyTurnDriverPort,
    },
};

/// RAII guard that ensures a checked-out [`MontySession`] is always returned to
/// the [`MontySessionRegistry`] when the guard is dropped, even if the future
/// driving the session is cancelled mid-flight (e.g. `TurnTimeout`,
/// `WorkerCancelled`, `DriverPanic`).
///
/// Call [`SessionGuard::take`] to move the session out for explicit handling
/// (park or drop). When `take()` is called the guard is disarmed and its `Drop`
/// is a no-op.
///
/// # Why this matters
/// `TurnRunnerWorker::execute_claimed_run` drives the Monty driver inside a
/// `tokio::select!`. If any non-driver arm wins (timeout, cancel, panic), the
/// `drive_turn` future is dropped. Without this guard, the session is silently
/// lost: the registry slot becomes empty, and the next turn restarts the VM
/// from scratch — silently discarding all cross-turn orchestrator state.
#[cfg(feature = "skills-db")]
struct SessionGuard {
    session: Option<MontySession>,
    registry: Arc<MontySessionRegistry>,
    scope: TurnScope,
}

#[cfg(feature = "skills-db")]
impl SessionGuard {
    fn new(session: MontySession, registry: Arc<MontySessionRegistry>, scope: TurnScope) -> Self {
        Self {
            session: Some(session),
            registry,
            scope,
        }
    }

    /// Take ownership of the session, disarming the park-on-drop guard.
    /// The caller must handle the session (park or drop it explicitly).
    fn take(mut self) -> MontySession {
        self.session
            .take()
            .expect("SessionGuard: session already taken")
    }
}

#[cfg(feature = "skills-db")]
impl Drop for SessionGuard {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            // The guard is being dropped while still holding the session — this
            // happens when the drive future is cancelled mid-flight. Re-park the
            // session so the next turn for this conversation can resume rather than
            // restarting the VM from scratch.
            //
            // `MontySessionRegistry::park` is async, but `Drop` is sync. We use
            // `tokio::runtime::Handle::try_current` + `spawn_blocking` / `block_in_place`
            // as a best-effort fallback. If no Tokio runtime is available (e.g. during
            // test teardown), the session is simply lost — acceptable since in practice
            // this guard only lives inside an async Tokio task.
            let registry = Arc::clone(&self.registry);
            let scope = self.scope.clone();
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn(async move {
                        registry.park(scope, session).await;
                    });
                }
                Err(_) => {
                    // No Tokio runtime — session is lost. This should not happen in
                    // production (the driver runs inside a Tokio task) but is safe to
                    // ignore: worst case is a fresh VM on the next turn.
                }
            }
        }
    }
}

/// Per-conversation signal-channel broker (user-locked A). Holds the
/// `SignalSender` for the turn currently in flight for each conversation so the
/// turn runner (slice 4d) can forward `Stop` / `Suspend` / `InjectMessage` into
/// `host.check_signals` mid-drive. A session is driven outside the registry
/// lock; the broker only ever holds the sender for the in-flight turn, cleared
/// when `drive_turn` returns.
#[cfg(feature = "skills-db")]
pub(crate) struct SignalBroker {
    senders: Mutex<HashMap<TurnScope, SignalSender>>,
}

#[cfg(feature = "skills-db")]
impl SignalBroker {
    pub(crate) fn new() -> Self {
        Self {
            senders: Mutex::new(HashMap::new()),
        }
    }

    /// Register the signal sender for `scope`'s in-flight turn. Overwrites any
    /// stale sender for the same scope (a prior turn that crashed without
    /// clearing).
    pub(crate) async fn set(&self, scope: TurnScope, tx: SignalSender) {
        self.senders.lock().await.insert(scope, tx);
    }

    /// Drop the signal sender for `scope` (the turn ended).
    pub(crate) async fn remove(&self, scope: &TurnScope) {
        self.senders.lock().await.remove(scope);
    }

    /// Forward `signal` to the in-flight turn for `scope`. No-op when no turn is
    /// in flight for that conversation. The sender is cloned out of the lock
    /// before awaiting the send so the lock is never held across the await.
    ///
    /// Called from slice 4d: the turn runner forwards `Stop`/`Suspend`/
    /// `InjectMessage` signals into the in-flight turn via this method.
    #[allow(dead_code)]
    pub(crate) async fn send(&self, scope: &TurnScope, signal: ThreadSignal) {
        let tx = {
            let senders = self.senders.lock().await;
            senders.get(scope).cloned()
        };
        if let Some(tx) = tx {
            let _ = tx.send(signal).await;
        }
    }
}

#[cfg(feature = "skills-db")]
impl Default for SignalBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// The cross-turn-persistent Monty orchestrator turn driver. Constructed once
/// at runtime-wiring time (slice 4d) and shared via `Arc<dyn
/// MontyTurnDriverPort>`. All mutable state (the session registry + signal
/// broker) is behind `Arc`/`Mutex`, so `drive_turn` takes `&self`.
#[cfg(feature = "skills-db")]
pub(crate) struct PersistentMontyDriver {
    registry: Arc<MontySessionRegistry>,
    signal_broker: Arc<SignalBroker>,
    /// Canonical loader for the live engine [`Thread`] AND the shared-memory-docs
    /// store passed to `prepare_monty_session` / `drive_to_yield`.
    store: Arc<dyn Store>,
    /// Per-run [`EffectExecutor`] builder (build_for_run is called once per
    /// [`drive_turn`] to snapshot the extension surface for the current run
    /// context, matching the pattern used by `PgOrchestratorLookup::run_tier_zero`).
    effect_executor_builder: Arc<TierZeroEffectExecutorBuilder>,
    leases: Arc<LeaseManager>,
    policy: Arc<PolicyEngine>,
    event_tx: Option<tokio::sync::broadcast::Sender<ThreadEvent>>,
    gate_controller: Arc<dyn GateController>,
    dynamic_tools: Option<Arc<dyn DynamicToolPort>>,
    component_port: Option<Arc<dyn ComponentPort>>,
    kohai_port: Option<Arc<dyn KohaiPort>>,
    /// DB-backed max wall-clock budget override for the Monty VM.
    max_duration_secs: Option<u64>,
    /// Thread service used to persist assistant messages added by
    /// `host.post_reply` during a Monty turn (C.6 slice 4d reply persistence).
    session_thread_service: Arc<dyn SessionThreadService>,
}

#[cfg(feature = "skills-db")]
impl PersistentMontyDriver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        registry: Arc<MontySessionRegistry>,
        signal_broker: Arc<SignalBroker>,
        store: Arc<dyn Store>,
        effect_executor_builder: Arc<TierZeroEffectExecutorBuilder>,
        leases: Arc<LeaseManager>,
        policy: Arc<PolicyEngine>,
        event_tx: Option<tokio::sync::broadcast::Sender<ThreadEvent>>,
        gate_controller: Arc<dyn GateController>,
        dynamic_tools: Option<Arc<dyn DynamicToolPort>>,
        component_port: Option<Arc<dyn ComponentPort>>,
        kohai_port: Option<Arc<dyn KohaiPort>>,
        max_duration_secs: Option<u64>,
        session_thread_service: Arc<dyn SessionThreadService>,
    ) -> Self {
        Self {
            registry,
            signal_broker,
            store,
            effect_executor_builder,
            leases,
            policy,
            event_tx,
            gate_controller,
            dynamic_tools,
            component_port,
            kohai_port,
            max_duration_secs,
            session_thread_service,
        }
    }

    /// Load the live engine [`Thread`] for `context.thread_id`, mapping the
    /// turns `ThreadId` → engine `ThreadId(pub Uuid)`. Returns `None` on a
    /// parse failure, a store miss, or a store error — the caller degrades to a
    /// `Failed` exit. Mirrors `PgOrchestratorLookup::load_thread`.
    async fn load_thread(&self, context: &LoopRunContext) -> Option<Thread> {
        let uuid = match uuid::Uuid::parse_str(context.thread_id.as_str()) {
            Ok(uuid) => uuid,
            Err(error) => {
                debug!(%error, "PersistentMontyDriver::load_thread: thread_id not a uuid; degrading to None");
                return None;
            }
        };
        match self.store.load_thread(EngineThreadId(uuid)).await {
            Ok(Some(thread)) => Some(thread),
            Ok(None) => None,
            Err(error) => {
                debug!(%error, "PersistentMontyDriver::load_thread failed; degrading to None");
                None
            }
        }
    }

    /// Drive one yield of the session, converting a `&str` user input into the
    /// `MontyObject::String` `drive_to_yield` expects. Centralizes the 14-dep
    /// forwarding so the prime/resume callers stay one-liners. `user_input =
    /// None` is the turn-1 prime; `Some(s)` is the resume.
    async fn drive_one(
        &self,
        session: &mut MontySession,
        thread: &mut Thread,
        signal_rx: &mut SignalReceiver,
        effects: &Arc<dyn EffectExecutor>,
        user_input: Option<&str>,
    ) -> Result<OrchestratorYield, AgentLoopDriverError> {
        let new_input = user_input.map(|s| MontyObject::String(s.to_string()));
        session
            .drive_to_yield(
                thread,
                effects,
                &self.leases,
                &self.policy,
                signal_rx,
                self.event_tx.as_ref(),
                Some(&self.store),
                &self.gate_controller,
                self.dynamic_tools.as_ref(),
                self.component_port.as_ref(),
                self.kohai_port.as_ref(),
                new_input,
            )
            .await
            .map_err(|e| AgentLoopDriverError::Failed {
                reason_kind: format!("monty turn driver: drive failed: {e}"),
            })
    }

    /// Core turn logic, factored out of the trait method so it takes only the
    /// run context (no `host`) — the host is consulted solely for
    /// `run_context()` in [`Self::drive_turn`].
    async fn drive_turn_inner(
        &self,
        context: &LoopRunContext,
        thread: &mut Thread,
        signal_rx: &mut SignalReceiver,
        user_input: String,
        max_duration_override: Option<std::time::Duration>,
    ) -> Result<LoopExit, AgentLoopDriverError> {
        // Build the per-run EffectExecutor (snapshot extension surface for this
        // turn's run context — same pattern as PgOrchestratorLookup::run_tier_zero).
        let effects = self
            .effect_executor_builder
            .build_for_run(context)
            .await
            .map_err(|e| AgentLoopDriverError::Failed {
                reason_kind: format!("monty turn driver: build_for_run failed: {e}"),
            })?;

        // Checkout a parked session for this conversation, or build a fresh one
        // (turn 1). Turn 1 needs a prime drive (None) to reach the first
        // host.await_next_turn() park, THEN the resume drive (Some(input)).
        //
        // The session is wrapped in a `SessionGuard` immediately after checkout /
        // construction. The guard ensures the session is re-parked if this future
        // is cancelled mid-flight (e.g. TurnTimeout, WorkerCancelled, DriverPanic)
        // — without it the session would be silently lost and the next turn for
        // this conversation would restart the VM from scratch.
        let session = match self.registry.try_checkout(&context.scope).await {
            Some(session) => {
                SessionGuard::new(session, Arc::clone(&self.registry), context.scope.clone())
            }
            None => {
                let mut fresh =
                    prepare_monty_session(thread, Some(&self.store), max_duration_override)
                        .await
                        .map_err(|e| AgentLoopDriverError::Failed {
                            reason_kind: format!("monty turn driver: prepare session failed: {e}"),
                        })?;
                // Prime: drive the fresh VM to its first await_next_turn() park.
                match self
                    .drive_one(&mut fresh, thread, signal_rx, &effects, None)
                    .await?
                {
                    OrchestratorYield::AwaitNextTurn => {}
                    OrchestratorYield::Complete(_) => {
                        // The orchestrator terminated before processing any
                        // input (e.g. a stop signal at turn start). The session
                        // is done — do not park.
                        return self.completed_exit(context);
                    }
                }
                SessionGuard::new(fresh, Arc::clone(&self.registry), context.scope.clone())
            }
        };
        // Move the session out of the guard for driving; re-wrap afterwards.
        // `take()` disarms the guard so it won't double-park on drop.
        let mut session = session.take();

        // Snapshot message count before the resume drive so we can diff
        // afterwards and persist any new assistant messages from host.post_reply.
        let pre_resume_message_count = thread.messages.len();

        // Resume the parked await_next_turn() with this turn's user input.
        let yield_ = match self
            .drive_one(&mut session, thread, signal_rx, &effects, Some(&user_input))
            .await
        {
            Ok(y) => y,
            Err(e) => {
                // Drive failed — the VM state is unknown. Do not re-park; discard
                // the session so the next turn starts fresh rather than handing a
                // potentially-corrupt VM to the next resume.
                drop(session);
                return Err(e);
            }
        };

        // The drive succeeded. Determine the disposition of the session before
        // persisting, so a persistence failure cannot strand the session.
        match yield_ {
            OrchestratorYield::Complete(_) => {
                // VM finished — discard the session (it must not be reused).
                drop(session);
            }
            OrchestratorYield::AwaitNextTurn => {
                // Turn done, VM stays alive — park now, before persisting.
                // Parking first means a persistence failure below cannot lose VM
                // state: the session is safely stored and the next turn can resume.
                self.registry.park(context.scope.clone(), session).await;
            }
        }

        // Persist assistant messages appended by host.post_reply during the resume.
        // This is the "persisted outside the LoopExit ref mechanism" step documented
        // in the module-level comment: the orchestrator owns the reply artifact,
        // and we write it to SessionThreadService here so the WebUI can display it.
        self.persist_new_assistant_messages(context, thread, pre_resume_message_count)
            .await?;

        self.completed_exit(context)
    }

    /// Persist any new assistant messages added to `thread.messages` after
    /// index `pre_drive_message_count` by `host.post_reply` during the resume
    /// drive. For each new assistant message, calls `append_assistant_draft`
    /// then `finalize_assistant_message` on the [`SessionThreadService`].
    ///
    /// Returns an error if persistence fails for any message — a failed
    /// persist means the reply is lost, which is treated as a driver failure.
    async fn persist_new_assistant_messages(
        &self,
        context: &LoopRunContext,
        thread: &Thread,
        pre_drive_message_count: usize,
    ) -> Result<(), AgentLoopDriverError> {
        let new_messages = if pre_drive_message_count < thread.messages.len() {
            &thread.messages[pre_drive_message_count..]
        } else {
            return Ok(());
        };

        let thread_scope = thread_scope_from_turn_scope(&context.scope);
        let thread_id = context.scope.thread_id.clone();
        let run_id_str = context.run_id.to_string();

        for msg in new_messages {
            if msg.role != MessageRole::Assistant || msg.content.is_empty() {
                continue;
            }
            let draft = self
                .session_thread_service
                .append_assistant_draft(AppendAssistantDraftRequest {
                    scope: thread_scope.clone(),
                    thread_id: thread_id.clone(),
                    turn_run_id: run_id_str.clone(),
                    content: MessageContent::text(msg.content.clone()),
                })
                .await
                .map_err(|e| AgentLoopDriverError::Failed {
                    reason_kind: format!(
                        "monty turn driver: failed to append assistant draft: {e}"
                    ),
                })?;
            self.session_thread_service
                .finalize_assistant_message(
                    &thread_scope,
                    &thread_id,
                    draft.message_id,
                    MessageContent::text(msg.content.clone()),
                )
                .await
                .map_err(|e| AgentLoopDriverError::Failed {
                    reason_kind: format!(
                        "monty turn driver: failed to finalize assistant message: {e}"
                    ),
                })?;
            debug!(
                run_id = %context.run_id,
                "PersistentMontyDriver: persisted assistant reply from host.post_reply"
            );
        }
        Ok(())
    }

    /// Minimal completed-exit handshake: the orchestrator owns the durable
    /// reply artifact (`host.post_reply`), so the exit carries no refs.
    fn completed_exit(&self, context: &LoopRunContext) -> Result<LoopExit, AgentLoopDriverError> {
        let exit_id = completed_exit_id(&context.run_id)?;
        Ok(LoopExit::Completed(LoopCompleted {
            completion_kind: LoopCompletionKind::NoReply,
            reply_message_refs: Vec::new(),
            result_refs: Vec::new(),
            final_checkpoint_id: None,
            usage_summary_ref: None,
            exit_id,
        }))
    }
}

/// Build the [`LoopExitId`] for a completed turn (`exit:<run_id>-completed`).
/// Mirrors `brassclaw_agent_loop::executor::exit_helpers::exit_id`.
#[cfg(feature = "skills-db")]
fn completed_exit_id(run_id: &TurnRunId) -> Result<LoopExitId, AgentLoopDriverError> {
    LoopExitId::new(format!("exit:{run_id}-completed")).map_err(|_| AgentLoopDriverError::Failed {
        reason_kind: "run id could not be represented as loop exit id".to_string(),
    })
}

/// Build a [`ThreadScope`] from a [`TurnScope`] for use with
/// [`SessionThreadService`] calls. `agent_id` falls back to `"default"` when
/// the `TurnScope` does not carry an explicit agent (tenant-level turns).
/// `owner_user_id` is extracted from the explicit thread owner when present.
#[cfg(feature = "skills-db")]
fn thread_scope_from_turn_scope(scope: &TurnScope) -> ThreadScope {
    ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id: scope
            .agent_id
            .clone()
            .unwrap_or_else(|| brassclaw_host_api::AgentId::from_trusted("default".to_string())),
        project_id: scope.project_id.clone(),
        owner_user_id: scope.explicit_owner_user_id().cloned(),
    }
}

#[cfg(feature = "skills-db")]
#[async_trait]
impl MontyTurnDriverPort for PersistentMontyDriver {
    async fn drive_turn(
        &self,
        _request: AgentLoopDriverRunRequest,
        host: &(dyn AgentLoopDriverHost + Send + Sync),
    ) -> Result<LoopExit, AgentLoopDriverError> {
        let context = host.run_context();
        let scope = context.scope.clone();

        let thread =
            self.load_thread(context)
                .await
                .ok_or_else(|| AgentLoopDriverError::Failed {
                    reason_kind: "monty turn driver: thread not found".to_string(),
                })?;
        let mut thread = thread;

        // Load the last finalized User message from the session thread service.
        // `PgThreadEngineStore::map_record` always returns an empty `messages`
        // vec (it only maps metadata, not transcript rows), so reading
        // `last_user_input_string` from `thread.messages` returns `""` on every
        // turn — causing the VM to hit the empty-input `FINAL(...)` path instead
        // of processing the actual user message.
        let thread_scope = thread_scope_from_turn_scope(&context.scope);
        let user_input = match self
            .session_thread_service
            .latest_thread_message(LatestThreadMessageRequest {
                scope: thread_scope,
                thread_id: context.scope.thread_id.clone(),
                kind: MessageKind::User,
                // By the time the driver runs the inbound turn coordinator has
                // transitioned the User message from Accepted → Submitted.
                status: MessageStatus::Submitted,
            })
            .await
        {
            Ok(Some(record)) => record.content.unwrap_or_default(),
            Ok(None) => {
                debug!(
                    run_id = %context.run_id,
                    "PersistentMontyDriver: no finalized user message found; using empty input"
                );
                String::new()
            }
            Err(error) => {
                debug!(
                    run_id = %context.run_id,
                    %error,
                    "PersistentMontyDriver: failed to load last user message; using empty input"
                );
                String::new()
            }
        };
        let max_duration_override = self.max_duration_secs.map(std::time::Duration::from_secs);

        // Per-turn signal channel: the broker holds the sender so the turn
        // runner (slice 4d) can forward Stop/Suspend/Inject; the receiver feeds
        // host.check_signals inside drive_to_yield.
        let (signal_tx, mut signal_rx) = signal_channel(32);
        self.signal_broker.set(scope.clone(), signal_tx).await;

        let result = self
            .drive_turn_inner(
                context,
                &mut thread,
                &mut signal_rx,
                user_input,
                max_duration_override,
            )
            .await;

        // Turn is over either way: drop the turn's signal sender so a stray
        // forward can't queue into a dead receiver.
        self.signal_broker.remove(&scope).await;

        result
    }
}

#[cfg(all(test, feature = "skills-db"))]
mod tests {
    use super::*;
    use brassclaw_host_api::{ProjectId, TenantId, ThreadId};

    fn test_scope(suffix: &str) -> TurnScope {
        TurnScope::new(
            TenantId::new("tenant").unwrap(),
            None,
            Some(ProjectId::new("project").unwrap()),
            ThreadId::new(format!("00000000-0000-0000-0000-0000000000{suffix}")).unwrap(),
        )
    }

    #[tokio::test]
    async fn signal_broker_set_then_send_delivers_to_receiver() {
        let broker = SignalBroker::new();
        let scope = test_scope("01");
        let (tx, mut rx) = signal_channel(32);
        broker.set(scope.clone(), tx).await;

        broker.send(&scope, ThreadSignal::Stop).await;

        let received = rx.recv().await.expect("signal delivered");
        assert!(matches!(received, ThreadSignal::Stop));
    }

    #[tokio::test]
    async fn signal_broker_remove_drops_sender_so_recv_returns_none() {
        let broker = SignalBroker::new();
        let scope = test_scope("02");
        let (tx, mut rx) = signal_channel(32);
        broker.set(scope.clone(), tx).await;
        broker.remove(&scope).await;

        broker.send(&scope, ThreadSignal::Stop).await;
        // The sender was removed from the broker; the only remaining sender is
        // the one we moved into `set` (now dropped on remove). With all senders
        // dropped, `recv` returns `None` (channel closed).
        let received = rx.recv().await;
        assert!(received.is_none(), "channel should be closed after remove");
    }

    #[tokio::test]
    async fn signal_broker_send_with_no_entry_is_a_noop() {
        let broker = SignalBroker::new();
        let scope = test_scope("03");
        // No `set` — send must not panic and must not block.
        broker.send(&scope, ThreadSignal::Stop).await;
        assert!(broker.senders.lock().await.is_empty());
    }

    #[test]
    fn completed_exit_id_builds_run_id_scoped_exit_id() {
        let run_id = TurnRunId::new();
        let exit_id = completed_exit_id(&run_id).expect("valid run id");
        assert_eq!(exit_id.as_str(), format!("exit:{run_id}-completed"));
    }

    #[test]
    fn persistent_monty_driver_is_send_sync() {
        // De-risks slice 4d: the driver is shared via `Arc<dyn
        // MontyTurnDriverPort>` across workers, so it must be `Send + Sync`.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PersistentMontyDriver>();
        assert_send_sync::<SignalBroker>();
    }

    // ── SessionGuard ──────────────────────────────────────────────────────────

    #[test]
    fn session_guard_is_send() {
        // The guard is moved into async blocks inside drive_turn_inner (across
        // await points), so it must be Send. MontySession is Send but !Sync;
        // SessionGuard inherits !Sync, which is acceptable — it is never shared
        // via &SessionGuard across threads, only moved across await points.
        fn assert_send<T: Send>() {}
        assert_send::<SessionGuard>();
    }
}
