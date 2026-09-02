//! Thread manager — top-level orchestrator for thread lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::capability::lease::LeaseManager;
use crate::capability::planner::LeasePlanner;
use crate::capability::policy::PolicyEngine;
use crate::capability::registry::CapabilityRegistry;
use crate::executor::ExecutionLoop;
use crate::runtime::lease_refresh::reconcile_dynamic_tool_lease;
use crate::runtime::messaging::{self, SignalSender, ThreadOutcome, ThreadSignal};
use crate::runtime::tree::ThreadTree;
use crate::traits::effect::EffectExecutor;
use crate::traits::llm::LlmBackend;
use crate::traits::store::Store;
use crate::types::error::EngineError;
use crate::types::message::{MessageRole, ThreadMessage};
use crate::types::project::ProjectId;
use crate::types::thread::{Thread, ThreadConfig, ThreadId, ThreadState, ThreadType};

/// Handle to a running thread for checking results.
struct RunningThread {
    signal_tx: SignalSender,
    handle: tokio::task::JoinHandle<Result<ThreadOutcome, EngineError>>,
}

/// Top-level orchestrator for thread lifecycle.
///
/// Manages thread spawning, supervision, signaling, and tree relationships.
pub struct ThreadManager {
    llm: Arc<dyn LlmBackend>,
    effects: Arc<dyn EffectExecutor>,
    store: Arc<dyn Store>,
    pub capabilities: Arc<CapabilityRegistry>,
    pub leases: Arc<LeaseManager>,
    pub policy: Arc<PolicyEngine>,
    lease_planner: LeasePlanner,
    tree: RwLock<ThreadTree>,
    running: Arc<RwLock<HashMap<ThreadId, RunningThread>>>,
    completed: Arc<RwLock<HashMap<ThreadId, ThreadOutcome>>>,
    /// Broadcast channel for thread events (for live status updates).
    event_tx: tokio::sync::broadcast::Sender<crate::types::event::ThreadEvent>,
    /// Host-supplied callback that turns `Approval` gates into inline
    /// awaits instead of unwinding the call stack. The engine attaches
    /// it to every `ThreadExecutionContext` so both Tier 0 and Tier 1
    /// executors can pause a live VM in place.
    ///
    /// Defaults to [`crate::gate::CancellingGateController`] — every
    /// gate cancels with a typed denial. Hosts that want real inline
    /// await call [`Self::set_gate_controller`] during bootstrap.
    gate_controller: tokio::sync::RwLock<Arc<dyn crate::gate::GateController>>,
    /// DB-backed max wall-clock budget for the Monty orchestrator VM.
    /// Loaded from `reborn_monty_vm_settings.max_duration_secs` at startup.
    /// `Some` overrides `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` (Step 9.3).
    /// `None` falls back to the env-var / compiled-in DB-less default.
    max_duration_secs: Option<u64>,
    /// Postgres pool for DB-backed skill + component loading (`skills-db`
    /// feature). Plumbed into every spawned `ExecutionLoop` via
    /// [`Self::with_pg_pool`] so the SEC-01-validated orchestrator host
    /// functions (`handle_list_skills`, `handle_fetch_component`,
    /// `handle_resolve_component_by_name`) can read `reborn_skills` /
    /// `reborn_components` instead of falling back to the legacy in-memory
    /// `Store`. `None` (the default) keeps the legacy in-memory / `RamSource`
    /// behaviour.
    #[cfg(feature = "skills-db")]
    pg_pool: Option<std::sync::Arc<brassclaw_pg::PgPool>>,
}

impl ThreadManager {
    pub fn new(
        llm: Arc<dyn LlmBackend>,
        effects: Arc<dyn EffectExecutor>,
        store: Arc<dyn Store>,
        capabilities: Arc<CapabilityRegistry>,
        leases: Arc<LeaseManager>,
        policy: Arc<PolicyEngine>,
    ) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            llm,
            effects,
            store,
            capabilities,
            leases,
            policy,
            lease_planner: LeasePlanner::new(),
            tree: RwLock::new(ThreadTree::new()),
            running: Arc::new(RwLock::new(HashMap::new())),
            completed: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            gate_controller: tokio::sync::RwLock::new(crate::gate::CancellingGateController::arc()),
            max_duration_secs: None,
            #[cfg(feature = "skills-db")]
            pg_pool: None,
        }
    }

    /// Set the DB-backed max wall-clock budget for the Monty orchestrator VM.
    ///
    /// Call this after loading `MontyVmSettings` from `reborn_monty_vm_settings`
    /// so subsequent thread spawns use the DB-persisted value instead of the
    /// env-var / compiled-in DB-less fallback.
    pub fn with_max_duration_secs(mut self, secs: u64) -> Self {
        self.max_duration_secs = Some(secs);
        self
    }

    /// Set a Postgres pool for DB-backed skill + component loading
    /// (`skills-db` feature). The pool is plumbed into every spawned
    /// [`ExecutionLoop`] so the SEC-01-validated orchestrator host functions
    /// (`handle_list_skills`, `handle_fetch_component`,
    /// `handle_resolve_component_by_name`) can read `reborn_skills` /
    /// `reborn_components` instead of falling back to the legacy in-memory
    /// `Store`. Hosts without a pool keep the legacy in-memory behaviour.
    #[cfg(feature = "skills-db")]
    pub fn with_pg_pool(mut self, pool: std::sync::Arc<brassclaw_pg::PgPool>) -> Self {
        self.pg_pool = Some(pool);
        self
    }

    /// Install (or replace) the host-supplied gate controller.
    ///
    /// Called once during bridge bootstrap. Subsequent thread spawns
    /// pick up the controller and propagate it into every
    /// `ThreadExecutionContext` they construct.
    pub async fn set_gate_controller(&self, controller: Arc<dyn crate::gate::GateController>) {
        *self.gate_controller.write().await = controller;
    }

    /// Snapshot the current gate controller.
    pub async fn gate_controller(&self) -> Arc<dyn crate::gate::GateController> {
        self.gate_controller.read().await.clone()
    }

    /// Subscribe to thread events for live status updates.
    pub fn subscribe_events(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::types::event::ThreadEvent> {
        self.event_tx.subscribe()
    }

    /// Spawn a new thread and start executing it.
    ///
    /// Grants default capability leases for all registered capabilities.
    /// Returns the thread ID immediately; the thread runs in a background task.
    ///
    /// `initial_messages` provides conversation history from prior threads
    /// (for context continuity across turns in the same conversation).
    pub async fn spawn_thread(
        &self,
        goal: impl Into<String>,
        thread_type: ThreadType,
        project_id: ProjectId,
        config: ThreadConfig,
        parent_id: Option<ThreadId>,
        user_id: impl Into<String>,
    ) -> Result<ThreadId, EngineError> {
        self.spawn_thread_with_history(
            goal,
            None,
            thread_type,
            project_id,
            config,
            parent_id,
            user_id,
            Vec::new(),
            serde_json::Map::new(),
        )
        .await
    }

    /// Spawn a new thread with an explicit sidebar title.
    ///
    /// Callers with a semantic short label (e.g. mission name) should
    /// use this; everything else can rely on `spawn_thread` + the
    /// read-side fallback that derives a short title from `goal`.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn_thread_with_title(
        &self,
        goal: impl Into<String>,
        title: Option<String>,
        thread_type: ThreadType,
        project_id: ProjectId,
        config: ThreadConfig,
        parent_id: Option<ThreadId>,
        user_id: impl Into<String>,
    ) -> Result<ThreadId, EngineError> {
        self.spawn_thread_with_history(
            goal,
            title,
            thread_type,
            project_id,
            config,
            parent_id,
            user_id,
            Vec::new(),
            serde_json::Map::new(),
        )
        .await
    }

    /// Spawn a thread with initial conversation history.
    ///
    /// `initial_metadata` is applied to the thread's metadata map *before* the
    /// background execution task starts, so the executor's in-memory `Thread`
    /// observes those keys on the first step. This is the only correct way to
    /// stamp metadata that the very first orchestrator step needs to read
    /// (e.g. `source_channel` for `mission_create` notify-channel defaulting,
    /// or `user_timezone` for cron resolution). Setting metadata after spawn
    /// via `set_thread_metadata` is a race — the spawned task owns its own
    /// in-memory copy of the `Thread`, and the late update only lands on the
    /// persisted copy that the running task never re-reads.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn_thread_with_history(
        &self,
        goal: impl Into<String>,
        title: Option<String>,
        thread_type: ThreadType,
        project_id: ProjectId,
        config: ThreadConfig,
        parent_id: Option<ThreadId>,
        user_id: impl Into<String>,
        initial_messages: Vec<crate::types::message::ThreadMessage>,
        initial_metadata: serde_json::Map<String, serde_json::Value>,
    ) -> Result<ThreadId, EngineError> {
        let user_id = user_id.into();
        let mut thread = Thread::new(goal, thread_type, project_id, &user_id, config);
        if let Some(pid) = parent_id {
            thread = thread.with_parent(pid);
        }
        // Set the title before save_thread + start_thread so the
        // executor's in-memory thread observes it atomically.
        thread.title = title;
        let thread_id = thread.id;

        // Apply initial metadata before save_thread + start_thread so the
        // executor's in-memory thread observes it on the first step.
        if !initial_metadata.is_empty()
            && let Some(obj) = thread.metadata.as_object_mut()
        {
            for (k, v) in initial_metadata {
                obj.insert(k, v);
            }
        }

        // Register in tree
        if let Some(pid) = parent_id {
            self.tree.write().await.add_child(pid, thread_id);
        }

        // Grant explicit capability leases based on thread type.
        for grant in self
            .lease_planner
            .plan_for_thread(thread_type, &self.capabilities)
        {
            let lease = self
                .leases
                .grant(
                    thread_id,
                    grant.capability_name,
                    grant.granted_actions,
                    None,
                    None,
                )
                .await?;
            self.store.save_lease(&lease).await?;
            thread.capability_leases.push(lease.id);
        }

        // Add conversation history from prior threads (for context continuity)
        for msg in initial_messages {
            thread.messages.push(msg);
        }

        // Add the goal as the current user message so the LLM has context
        thread.add_message(crate::types::message::ThreadMessage::user(&thread.goal));

        // Persist
        self.store.save_thread(&thread).await?;

        self.start_thread(thread, user_id, false).await
    }

    /// Resume a persisted waiting or suspended thread.
    pub async fn resume_thread(
        &self,
        thread_id: ThreadId,
        user_id: impl Into<String>,
        injected_message: Option<ThreadMessage>,
        approval_event: Option<(String, bool)>,
        resolved_call_id: Option<String>,
    ) -> Result<(), EngineError> {
        if self.is_running(thread_id).await {
            return Err(EngineError::Thread(
                crate::types::error::ThreadError::AlreadyRunning(thread_id),
            ));
        }

        let mut thread = self
            .store
            .load_thread(thread_id)
            .await?
            .ok_or(EngineError::ThreadNotFound(thread_id))?;

        // Tenant isolation: verify the requesting user owns this thread.
        let uid: String = user_id.into();
        if !thread.is_owned_by(&uid) {
            return Err(EngineError::AccessDenied {
                user_id: uid,
                entity: format!("thread {thread_id}"),
            });
        }

        if !matches!(
            thread.state,
            crate::types::thread::ThreadState::Waiting
                | crate::types::thread::ThreadState::Suspended
        ) {
            return Err(EngineError::Store {
                reason: format!(
                    "thread {thread_id} is not resumable from {:?}",
                    thread.state
                ),
            });
        }

        if let Some((call_id, approved)) = approval_event {
            let event = crate::types::event::ThreadEvent::new(
                thread_id,
                crate::types::event::EventKind::ApprovalReceived { call_id, approved },
            );
            let _ = self.event_tx.send(event.clone());
            thread.events.push(event);
            thread.updated_at = chrono::Utc::now();
        }

        if let Some(ref call_id) = resolved_call_id {
            let preserve_assistant_call = injected_message.as_ref().is_some_and(|message| {
                message.role == MessageRole::ActionResult
                    && message.action_call_id.as_deref() == Some(call_id.as_str())
            });
            thread.messages.retain(|existing| {
                if preserve_assistant_call {
                    !is_resolved_action_result_message(existing, call_id)
                } else {
                    !is_resolved_call_message(existing, call_id)
                }
            });
        }

        if let Some(message) = injected_message {
            thread.add_internal_message(message.clone());
            thread.add_message(message);
        }

        // Waiting threads paused on approval/auth should resume from the
        // newly injected context rather than replaying the old checkpointed
        // interrupt. Suspended threads keep their checkpoint for restart.
        if thread.state == crate::types::thread::ThreadState::Waiting
            && let Some(metadata) = thread.metadata.as_object_mut()
        {
            metadata.remove("runtime_checkpoint");
        }

        self.store.save_thread(&thread).await?;
        self.start_thread(thread, uid, true).await?;
        Ok(())
    }

    async fn start_thread(
        &self,
        mut thread: Thread,
        user_id: String,
        is_resume: bool,
    ) -> Result<ThreadId, EngineError> {
        let thread_id = thread.id;

        reconcile_dynamic_tool_lease(
            &mut thread,
            &self.effects,
            &self.leases,
            Some(&self.store),
            &self.lease_planner,
        )
        .await?;

        // Create signal channel
        let (tx, rx) = messaging::signal_channel(32);

        // Build execution loop
        let llm = Arc::clone(&self.llm);
        let effects = Arc::clone(&self.effects);
        let leases = Arc::clone(&self.leases);
        let policy = Arc::clone(&self.policy);

        let store_for_retrieval = Arc::clone(&self.store);
        let retrieval = crate::memory::RetrievalEngine::new(Arc::clone(&store_for_retrieval));
        // TODO(Phase K): RamSource is the active retrieval backend. Replace with
        // PostgresSource once the composition layer wires it via with_retrieval_source().
        // PostgresSource requires a pg_pool; it is instantiated in the composition layer
        // and must override this default. Until Phase K, the intent system's PostgresSource
        // path is NOT active in production — only the RamSource keyword fallback runs.
        let retrieval_source: Arc<dyn crate::memory::RetrievalSource> =
            Arc::new(crate::memory::RamSource::new(store_for_retrieval));

        let gate_controller = self.gate_controller.read().await.clone();
        let mut exec_loop = ExecutionLoop::new(
            thread,
            llm,
            effects,
            leases,
            policy,
            rx,
            user_id,
            gate_controller,
        )
        .with_capabilities(Arc::clone(&self.capabilities))
        .with_event_tx(self.event_tx.clone())
        .with_retrieval(retrieval)
        .with_store(Arc::clone(&self.store))
        .with_retrieval_source(retrieval_source);
        // v3 Phase H4.8: plumb the DB pool into the ExecutionLoop so the
        // SEC-01-validated orchestrator host functions can read
        // `reborn_skills` / `reborn_components` instead of falling back to
        // the legacy in-memory `Store`.
        #[cfg(feature = "skills-db")]
        {
            if let Some(pool) = self.pg_pool.clone() {
                exec_loop = exec_loop.with_pg_pool(pool);
            }
        }
        // Thread DB-backed max duration into the execution loop (Step 9.3).
        if let Some(secs) = self.max_duration_secs {
            exec_loop = exec_loop.with_max_duration_secs(secs);
        }

        // Spawn background task
        let store_for_task = Arc::clone(&self.store);
        let running = Arc::clone(&self.running);
        let completed = Arc::clone(&self.completed);
        let handle = tokio::spawn(async move {
            let mut exec = exec_loop;
            let result = exec.run().await;
            debug!(thread_id = %thread_id, "thread execution finished");

            // Run retrospective trace analysis (non-LLM, always runs).
            // Issues are picked up by the self-improvement mission via event listener.
            let trace = crate::executor::trace::build_trace(&exec.thread);
            if !trace.issues.is_empty() {
                crate::executor::trace::log_trace_summary(&trace);
            }

            // Transition Completed → Done
            if exec.thread.state == crate::types::thread::ThreadState::Completed
                && let Err(e) = exec
                    .thread
                    .transition_to(crate::types::thread::ThreadState::Done, None)
            {
                tracing::debug!(thread_id = %thread_id, "failed to transition to Done: {e}");
            }

            // Trace recording is handled centrally by `RecordingLlm` in the
            // host crate (gated by `BRASSCLAW_RECORD_TRACE`). The engine no
            // longer writes its own JSON trace file.

            if let Err(e) = store_for_task.append_events(&exec.thread.events).await {
                tracing::debug!(
                    thread_id = %thread_id,
                    "failed to persist thread events: {e}"
                );
            }

            // Save final thread state to store
            if let Err(e) = store_for_task.save_thread(&exec.thread).await {
                tracing::debug!(
                    thread_id = %thread_id,
                    "failed to save final thread state: {e}"
                );
            }

            let outcome = match result {
                Ok(outcome) => outcome,
                Err(error) => {
                    let debug_detail = error.debug_detail().map(|s| s.to_string());
                    ThreadOutcome::Failed {
                        error: error.to_string(),
                        debug_detail,
                    }
                }
            };
            completed.write().await.insert(thread_id, outcome.clone());
            running.write().await.remove(&thread_id);
            Ok(outcome)
        });

        self.running.write().await.insert(
            thread_id,
            RunningThread {
                signal_tx: tx,
                handle,
            },
        );

        if is_resume {
            debug!(thread_id = %thread_id, "resumed thread");
        }

        Ok(thread_id)
    }

    /// Send a stop signal to a running thread.
    ///
    /// Wakes any [`crate::gate::GateController::pause`] futures that
    /// are currently parked on this thread BEFORE sending
    /// `ThreadSignal::Stop` so the engine task can observe the stop
    /// promptly. Without the explicit cancel, a thread parked inside
    /// `pause()` (inline approval await) is not polling the signal
    /// channel and would continue waiting until the user resolves the
    /// prompt or the gate expires.
    pub async fn stop_thread(&self, thread_id: ThreadId, user_id: &str) -> Result<(), EngineError> {
        // Validate ownership before allowing stop.
        if let Some(thread) = self.store.load_thread(thread_id).await?
            && !thread.is_owned_by(user_id)
        {
            return Err(EngineError::AccessDenied {
                user_id: user_id.to_string(),
                entity: format!("thread {thread_id}"),
            });
        }

        // Wake any inline gate await blocked on this thread first. The
        // controller is shared across spawned engine tasks; a parked
        // `pause()` future polled inside an executor doesn't see
        // `ThreadSignal::Stop` directly.
        let controller = self.gate_controller.read().await.clone();
        controller.cancel_thread(thread_id).await;

        let running = self.running.read().await;
        if let Some(rt) = running.get(&thread_id) {
            let _ = rt.signal_tx.send(ThreadSignal::Stop).await;
            Ok(())
        } else {
            Err(EngineError::ThreadNotFound(thread_id))
        }
    }

    /// Inject a user message into a running thread.
    pub async fn inject_message(
        &self,
        thread_id: ThreadId,
        user_id: &str,
        message: ThreadMessage,
    ) -> Result<(), EngineError> {
        // Validate ownership before allowing injection.
        if let Some(thread) = self.store.load_thread(thread_id).await?
            && !thread.is_owned_by(user_id)
        {
            return Err(EngineError::AccessDenied {
                user_id: user_id.to_string(),
                entity: format!("thread {thread_id}"),
            });
        }
        let running = self.running.read().await;
        if let Some(rt) = running.get(&thread_id) {
            let _ = rt
                .signal_tx
                .send(ThreadSignal::InjectMessage(message))
                .await;
            Ok(())
        } else {
            Err(EngineError::ThreadNotFound(thread_id))
        }
    }

    /// Set a metadata key on the persisted thread record.
    ///
    /// Note: this updates the **store**, not the in-memory `Thread` that an
    /// already-running `ExecutionLoop` is reading from. Callers that need the
    /// next executor step to observe the new value must apply this *before*
    /// the executor task is spawned (initial-create path) or before
    /// `resume_thread`, which reloads from the store.
    pub async fn set_thread_metadata(
        &self,
        thread_id: ThreadId,
        key: &str,
        value: &str,
    ) -> Result<(), EngineError> {
        let mut thread = self
            .store
            .load_thread(thread_id)
            .await
            .map_err(|e| EngineError::Store {
                reason: format!("set_thread_metadata: load failed: {e}"),
            })?
            .ok_or(EngineError::ThreadNotFound(thread_id))?;
        if let Some(obj) = thread.metadata.as_object_mut() {
            obj.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        self.store
            .save_thread(&thread)
            .await
            .map_err(|e| EngineError::Store {
                reason: format!("set_thread_metadata: save failed: {e}"),
            })?;
        Ok(())
    }

    /// Check if a thread is still running.
    pub async fn is_running(&self, thread_id: ThreadId) -> bool {
        let running = self.running.read().await;
        running
            .get(&thread_id)
            .is_some_and(|rt| !rt.handle.is_finished())
    }

    /// Wait for a thread to finish and return its outcome.
    /// Removes the thread from the running set.
    pub async fn join_thread(&self, thread_id: ThreadId) -> Result<ThreadOutcome, EngineError> {
        if let Some(outcome) = self.completed.write().await.remove(&thread_id) {
            return Ok(outcome);
        }

        let rt = {
            let mut running = self.running.write().await;
            running.remove(&thread_id)
        };

        match rt {
            Some(rt) => {
                let result = match rt.handle.await {
                    Ok(result) => result,
                    Err(e) => {
                        error!(thread_id = %thread_id, "thread task panicked: {e}");
                        Ok(ThreadOutcome::Failed {
                            error: format!("thread task panicked: {e}"),
                            debug_detail: None,
                        })
                    }
                };
                self.completed.write().await.remove(&thread_id);
                result
            }
            None => Err(EngineError::ThreadNotFound(thread_id)),
        }
    }

    /// Get children of a thread.
    pub async fn children_of(&self, thread_id: ThreadId) -> Vec<ThreadId> {
        let tree = self.tree.read().await;
        tree.children_of(thread_id).to_vec()
    }

    /// Get the parent of a thread.
    pub async fn parent_of(&self, thread_id: ThreadId) -> Option<ThreadId> {
        let tree = self.tree.read().await;
        tree.parent_of(thread_id)
    }

    /// Clean up finished threads from the running set.
    pub async fn cleanup_finished(&self) -> Vec<ThreadId> {
        let mut running = self.running.write().await;
        let finished: Vec<ThreadId> = running
            .iter()
            .filter(|(_, rt)| rt.handle.is_finished())
            .map(|(id, _)| *id)
            .collect();
        for id in &finished {
            running.remove(id);
        }
        finished
    }

    /// Automatically resume checkpointed non-foreground threads.
    pub async fn resume_background_threads(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ThreadId>, EngineError> {
        // System operation: resume all suspended research threads regardless of user.
        let threads = self.store.list_all_threads(project_id).await?;
        let mut resumed = Vec::new();

        for thread in threads {
            if thread.state != ThreadState::Suspended {
                continue;
            }
            if thread.thread_type != ThreadType::Research {
                continue;
            }
            if thread.metadata.get("runtime_checkpoint").is_none() {
                continue;
            }
            if thread.user_id.is_empty() {
                continue;
            }

            self.resume_thread(thread.id, thread.user_id.clone(), None, None, None)
                .await?;
            resumed.push(thread.id);
        }

        Ok(resumed)
    }

    /// Reconcile persisted non-terminal threads after process startup.
    ///
    /// The current engine does not support mid-thread replay/resume, so any
    /// thread left in a non-terminal state is marked failed-safe. Threads
    /// transitioned to `Failed` here carry the
    /// [`ENGINE_RESTART_RECOVERY_METADATA_KEY`] flag so callers can
    /// distinguish them from real, user-actionable failures.
    pub async fn recover_project_threads(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ThreadId>, EngineError> {
        // System operation: recover all non-terminal threads regardless of user.
        let threads = self.store.list_all_threads(project_id).await?;
        let mut recovered = Vec::new();

        for mut thread in threads {
            if thread.state.is_terminal() || thread.state == ThreadState::Completed {
                continue;
            }

            if thread.state == ThreadState::Waiting
                && thread.metadata.get(PENDING_APPROVAL_METADATA_KEY).is_some()
            {
                continue;
            }

            if thread
                .metadata
                .get(RUNTIME_CHECKPOINT_METADATA_KEY)
                .is_some()
                && matches!(thread.state, ThreadState::Running | ThreadState::Suspended)
            {
                if thread.state == ThreadState::Running {
                    thread.transition_to(
                        ThreadState::Suspended,
                        Some("engine restart; resumable from checkpoint".into()),
                    )?;
                }
                self.store.append_events(&thread.events).await?;
                self.store.save_thread(&thread).await?;
                recovered.push(thread.id);
                continue;
            }

            // Tag the thread before transitioning so downstream consumers
            // (projects "needs attention" feed, health rollup) can skip
            // restart-recovery noise and only surface real failures.
            if let Some(obj) = thread.metadata.as_object_mut() {
                obj.insert(
                    ENGINE_RESTART_RECOVERY_METADATA_KEY.to_string(),
                    serde_json::Value::Bool(true),
                );
            }

            if thread
                .transition_to(
                    ThreadState::Failed,
                    Some("engine restart before thread completion".into()),
                )
                .is_ok()
            {
                self.store.append_events(&thread.events).await?;
                self.store.save_thread(&thread).await?;
                recovered.push(thread.id);
            }
        }

        Ok(recovered)
    }
}

/// Metadata key set on a thread that has an in-flight pending-approval
/// gate. Persisted threads carrying this key skip restart-recovery so the
/// gate survives a process restart.
pub const PENDING_APPROVAL_METADATA_KEY: &str = "pending_approval";

/// Metadata key set on a thread that has a serialized runtime checkpoint
/// (CodeAct VM state, nudge counters, compaction count). Threads carrying
/// this key are suspended on restart instead of failed.
pub const RUNTIME_CHECKPOINT_METADATA_KEY: &str = "runtime_checkpoint";

/// Metadata key set on threads that were forced into `Failed` by
/// [`ThreadManager::recover_project_threads`] because the process
/// restarted before they could complete. The thread did not fail for
/// user-visible reasons; the projects "needs attention" surface filters
/// these out so an upgrade does not cascade into a wall of phantom
/// failure warnings.
pub const ENGINE_RESTART_RECOVERY_METADATA_KEY: &str = "engine_restart_recovery";

fn is_resolved_call_message(message: &ThreadMessage, call_id: &str) -> bool {
    if message.role == MessageRole::ActionResult
        && message.action_call_id.as_deref() == Some(call_id)
    {
        return true;
    }

    message.role == MessageRole::Assistant
        && message
            .action_calls
            .as_ref()
            .is_some_and(|calls| calls.iter().any(|call| call.id == call_id))
}

fn is_resolved_action_result_message(message: &ThreadMessage, call_id: &str) -> bool {
    message.role == MessageRole::ActionResult && message.action_call_id.as_deref() == Some(call_id)
}
