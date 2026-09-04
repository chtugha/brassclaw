//! Cross-turn-persistent Monty turn driver (C.6 slice 4c).
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
//! last-user-input extraction, the exit-id construction, and a `Send + Sync`
//! assert on the driver.

// Transient: the driver is landed ahead of its production consumer. C.6 slice 4d
// wires `PersistentMontyDriver` into the assembled runtime + `TurnRunnerWorker`
// direct path, at which point this allow should be removed.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use monty::MontyObject;
use tokio::sync::Mutex;
use tracing::debug;

use brassclaw_engine::{
    capability::{lease::LeaseManager, policy::PolicyEngine},
    executor::{
        orchestrator::{prepare_monty_session, MontySession, OrchestratorYield},
        ComponentPort, DynamicToolPort, KohaiPort,
    },
    gate::GateController,
    runtime::messaging::{signal_channel, SignalReceiver, SignalSender, ThreadSignal},
    traits::{effect::EffectExecutor, llm::LlmBackend},
    types::{
        event::ThreadEvent,
        message::{MessageRole, ThreadMessage},
        thread::{Thread, ThreadId as EngineThreadId},
    },
    Store,
};
use brassclaw_turns::{
    run_profile::{
        AgentLoopDriverError, AgentLoopDriverHost, AgentLoopDriverRunRequest, LoopRunContext,
        MontyTurnDriverPort,
    },
    LoopCompleted, LoopCompletionKind, LoopExit, LoopExitId, TurnRunId, TurnScope,
};

use crate::session_registry::MontySessionRegistry;

/// Per-conversation signal-channel broker (user-locked A). Holds the
/// `SignalSender` for the turn currently in flight for each conversation so the
/// turn runner (slice 4d) can forward `Stop` / `Suspend` / `InjectMessage` into
/// `host.check_signals` mid-drive. A session is driven outside the registry
/// lock; the broker only ever holds the sender for the in-flight turn, cleared
/// when `drive_turn` returns.
pub(crate) struct SignalBroker {
    senders: Mutex<HashMap<TurnScope, SignalSender>>,
}

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

impl Default for SignalBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// The cross-turn-persistent Monty orchestrator turn driver. Constructed once
/// at runtime-wiring time (slice 4d) and shared via `Arc<dyn
/// MontyTurnDriverPort>`. All mutable state (the session registry + signal
/// broker) is behind `Arc`/`Mutex`, so `drive_turn` takes `&self`.
pub(crate) struct PersistentMontyDriver {
    registry: Arc<MontySessionRegistry>,
    signal_broker: Arc<SignalBroker>,
    /// Canonical loader for the live engine [`Thread`] (mirrors
    /// `PgOrchestratorLookup::thread_store`) AND the shared-memory-docs store
    /// passed to `prepare_monty_session` / `drive_to_yield`.
    store: Arc<dyn Store>,
    llm: Arc<dyn LlmBackend>,
    effects: Arc<dyn EffectExecutor>,
    leases: Arc<LeaseManager>,
    policy: Arc<PolicyEngine>,
    event_tx: Option<tokio::sync::broadcast::Sender<ThreadEvent>>,
    gate_controller: Arc<dyn GateController>,
    dynamic_tools: Option<Arc<dyn DynamicToolPort>>,
    component_port: Option<Arc<dyn ComponentPort>>,
    kohai_port: Option<Arc<dyn KohaiPort>>,
    /// DB-backed max wall-clock budget override for the Monty VM.
    max_duration_secs: Option<u64>,
}

impl PersistentMontyDriver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        registry: Arc<MontySessionRegistry>,
        signal_broker: Arc<SignalBroker>,
        store: Arc<dyn Store>,
        llm: Arc<dyn LlmBackend>,
        effects: Arc<dyn EffectExecutor>,
        leases: Arc<LeaseManager>,
        policy: Arc<PolicyEngine>,
        event_tx: Option<tokio::sync::broadcast::Sender<ThreadEvent>>,
        gate_controller: Arc<dyn GateController>,
        dynamic_tools: Option<Arc<dyn DynamicToolPort>>,
        component_port: Option<Arc<dyn ComponentPort>>,
        kohai_port: Option<Arc<dyn KohaiPort>>,
        max_duration_secs: Option<u64>,
    ) -> Self {
        Self {
            registry,
            signal_broker,
            store,
            llm,
            effects,
            leases,
            policy,
            event_tx,
            gate_controller,
            dynamic_tools,
            component_port,
            kohai_port,
            max_duration_secs,
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
        user_input: Option<&str>,
    ) -> Result<OrchestratorYield, AgentLoopDriverError> {
        let new_input = user_input.map(|s| MontyObject::String(s.to_string()));
        session
            .drive_to_yield(
                thread,
                &self.llm,
                &self.effects,
                &self.leases,
                &self.policy,
                signal_rx,
                self.event_tx.as_ref(),
                // The three dead-walking deps (retrieval / platform_info /
                // _retrieval_source) are unused by the v3 host.* dispatch arms
                // and retire in C.7; passed as None here.
                None,
                Some(&self.store),
                None,
                &self.gate_controller,
                None,
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
        // Checkout a parked session for this conversation, or build a fresh one
        // (turn 1). Turn 1 needs a prime drive (None) to reach the first
        // host.await_next_turn() park, THEN the resume drive (Some(input)).
        let mut session = match self.registry.try_checkout(&context.scope).await {
            Some(session) => session,
            None => {
                let mut fresh =
                    prepare_monty_session(thread, Some(&self.store), max_duration_override)
                        .await
                        .map_err(|e| AgentLoopDriverError::Failed {
                            reason_kind: format!("monty turn driver: prepare session failed: {e}"),
                        })?;
                // Prime: drive the fresh VM to its first await_next_turn() park.
                match self
                    .drive_one(&mut fresh, thread, signal_rx, None)
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
                fresh
            }
        };

        // Resume the parked await_next_turn() with this turn's user input.
        let yield_ = self
            .drive_one(&mut session, thread, signal_rx, Some(&user_input))
            .await?;

        match yield_ {
            OrchestratorYield::Complete(_) => {
                // VM finished — drop the session (do not park).
                self.registry.drop_session(&context.scope).await;
            }
            OrchestratorYield::AwaitNextTurn => {
                // Turn done, VM stays alive — park for the next turn.
                self.registry.park(context.scope.clone(), session).await;
            }
        }
        self.completed_exit(context)
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
fn completed_exit_id(run_id: &TurnRunId) -> Result<LoopExitId, AgentLoopDriverError> {
    LoopExitId::new(format!("exit:{run_id}-completed")).map_err(|_| AgentLoopDriverError::Failed {
        reason_kind: "run id could not be represented as loop exit id".to_string(),
    })
}

/// The content of the last `User` message in `messages`, or `""` when there is
/// none. Mirrors `basic_mode.py::_last_user_input`.
fn last_user_input_from_messages(messages: &[ThreadMessage]) -> String {
    let mut last = String::new();
    for msg in messages {
        if msg.role == MessageRole::User {
            last = msg.content.clone();
        }
    }
    last
}

/// The current turn's user input: the last `User` message in the bootstrap
/// transcript the orchestrator consults (`internal_messages` when non-empty,
/// else the user-visible `messages`). Mirrors `build_orchestrator_inputs`'
/// `bootstrap_messages` choice so the input delivered via
/// `host.await_next_turn()` is the same message `_seed_history` dropped.
fn last_user_input_string(thread: &Thread) -> String {
    let messages = if thread.internal_messages.is_empty() {
        &thread.messages
    } else {
        &thread.internal_messages
    };
    last_user_input_from_messages(messages)
}

#[async_trait]
impl MontyTurnDriverPort for PersistentMontyDriver {
    async fn drive_turn(
        &self,
        _request: AgentLoopDriverRunRequest,
        host: &(dyn AgentLoopDriverHost + Send + Sync),
    ) -> Result<LoopExit, AgentLoopDriverError> {
        let context = host.run_context();
        let scope = context.scope.clone();

        let thread = self.load_thread(context).await.ok_or_else(|| {
            AgentLoopDriverError::Failed {
                reason_kind: "monty turn driver: thread not found".to_string(),
            }
        })?;
        let mut thread = thread;

        let user_input = last_user_input_string(&thread);
        let max_duration_override = self.max_duration_secs.map(std::time::Duration::from_secs);

        // Per-turn signal channel: the broker holds the sender so the turn
        // runner (slice 4d) can forward Stop/Suspend/Inject; the receiver feeds
        // host.check_signals inside drive_to_yield.
        let (signal_tx, mut signal_rx) = signal_channel(32);
        self.signal_broker.set(scope.clone(), signal_tx).await;

        let result = self
            .drive_turn_inner(context, &mut thread, &mut signal_rx, user_input, max_duration_override)
            .await;

        // Turn is over either way: drop the turn's signal sender so a stray
        // forward can't queue into a dead receiver.
        self.signal_broker.remove(&scope).await;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::types::message::ThreadMessage;
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
    fn last_user_input_from_messages_returns_last_user_content() {
        let messages = vec![
            ThreadMessage::system("system prompt"),
            ThreadMessage::user("first question"),
            ThreadMessage::system("assistant answer"),
            ThreadMessage::user("second question"),
        ];
        assert_eq!(last_user_input_from_messages(&messages), "second question");
    }

    #[test]
    fn last_user_input_from_messages_returns_empty_when_no_user_messages() {
        let messages = vec![
            ThreadMessage::system("system prompt"),
            ThreadMessage::system("assistant answer"),
        ];
        assert_eq!(last_user_input_from_messages(&messages), "");
    }

    #[test]
    fn last_user_input_from_messages_returns_empty_for_empty_transcript() {
        assert_eq!(last_user_input_from_messages(&[]), "");
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
}
