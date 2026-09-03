//! Core execution loop — the replacement for `run_agentic_loop()`.
//!
//! The `ExecutionLoop` owns a thread and drives it through LLM call →
//! action execution → result processing → repeat cycles. Unlike the
//! existing delegate pattern, the loop is self-contained: all behavior
//! differences between thread types are handled via capability leases
//! and policy, not delegate implementations.

use std::sync::Arc;

use tracing::debug;

use crate::capability::lease::LeaseManager;
use crate::capability::policy::PolicyEngine;
use crate::runtime::messaging::{SignalReceiver, ThreadOutcome};
use crate::traits::effect::EffectExecutor;
use crate::traits::llm::LlmBackend;
use crate::types::error::EngineError;
use crate::types::event::EventKind;
use crate::types::step::{Step, StepId};
use crate::types::thread::{Thread, ThreadState};

const RUNTIME_CHECKPOINT_METADATA_KEY: &str = "runtime_checkpoint";

/// Persisted state from a prior execution, used to resume threads.
/// The Python orchestrator manages loop counters internally; Rust only
/// needs the opaque `persisted_state` blob to hand back on resume.
#[derive(Default)]
struct RuntimeCheckpoint {
    persisted_state: serde_json::Value,
}

impl RuntimeCheckpoint {
    fn has_working_messages_system_prompt(&self) -> bool {
        self.persisted_state
            .get("working_messages")
            .and_then(|value| value.as_array())
            .is_some_and(|messages| {
                messages.iter().any(|message| {
                    let role = message.get("role").and_then(|value| value.as_str());
                    let content = message.get("content").and_then(|value| value.as_str());
                    matches!(role, Some("System" | "system"))
                        && content.is_some_and(crate::executor::prompt::is_codeact_system_prompt)
                })
            })
    }

    fn update_working_messages_system_prompt(&mut self, system_prompt: &str) -> bool {
        let Some(messages) = self
            .persisted_state
            .get_mut("working_messages")
            .and_then(|value| value.as_array_mut())
        else {
            return false;
        };

        if let Some(message) = messages.iter_mut().find(|message| {
            let role = message.get("role").and_then(|value| value.as_str());
            let content = message.get("content").and_then(|value| value.as_str());
            matches!(role, Some("System" | "system"))
                && content.is_some_and(crate::executor::prompt::is_codeact_system_prompt)
        }) {
            let refreshed = message
                .get("content")
                .and_then(|value| value.as_str())
                .map(|content| {
                    crate::executor::prompt::refresh_codeact_system_prompt(content, system_prompt)
                })
                .unwrap_or_else(|| system_prompt.to_string());
            if message
                .get("content")
                .and_then(|value| value.as_str())
                .is_some_and(|content| content == refreshed)
            {
                return false;
            }
            *message = serde_json::json!({
                "role": "System",
                "content": refreshed,
            });
            return true;
        }

        if messages.iter().any(|message| {
            matches!(
                message.get("role").and_then(|value| value.as_str()),
                Some("System" | "system")
            )
        }) {
            return false;
        }

        messages.insert(
            0,
            serde_json::json!({
                "role": "System",
                "content": system_prompt,
            }),
        );
        true
    }
}

/// The core execution loop for a thread.
pub struct ExecutionLoop {
    pub thread: Thread,
    llm: Arc<dyn LlmBackend>,
    effects: Arc<dyn EffectExecutor>,
    leases: Arc<LeaseManager>,
    policy: Arc<PolicyEngine>,
    signal_rx: SignalReceiver,
    /// Stored for potential future use (e.g. user-scoped prompt overlays).
    _user_id: String,
    /// Optional capability registry for resolving capability-level policies.
    capabilities: Option<Arc<crate::capability::registry::CapabilityRegistry>>,
    /// Optional broadcast sender for live event streaming.
    event_tx: Option<tokio::sync::broadcast::Sender<crate::types::event::ThreadEvent>>,
    /// Optional retrieval engine for injecting prior knowledge into context.
    retrieval: Option<crate::memory::RetrievalEngine>,
    /// Optional Store for runtime prompt overlay loading and skill retrieval.
    store: Option<Arc<dyn crate::traits::store::Store>>,
    /// Runtime platform metadata for self-awareness in system prompts.
    platform_info: Option<crate::executor::prompt::PlatformInfo>,
    /// Host gate controller, attached to every `ThreadExecutionContext`
    /// this loop builds so executors can pause in place on `Approval`
    /// gates. Required: callers without an inline-await surface use
    /// [`crate::gate::CancellingGateController::arc()`].
    gate_controller: Arc<dyn crate::gate::GateController>,
    /// Optional Postgres pool used by the `skills-db` feature to load skills
    /// from `reborn_skills` instead of MemoryDoc filesystem discovery.
    #[cfg(feature = "skills-db")]
    pg_pool: Option<std::sync::Arc<brassclaw_pg::PgPool>>,
    /// Phase 5 retrieval source (PostgresSource or RamSource). v3 Phase H8.4
    /// deleted the `__assemble_prior_knowledge__` dispatch arm that consumed
    /// this; the field stays plumbed through the dormant Model A dispatch fn
    /// until the final post-H.8 cleanup removes that fn entirely. The live
    /// Model B/C path consumes retrieval via `RetrievalTurnResult` (H.4).
    retrieval_source: Option<Arc<dyn crate::memory::RetrievalSource>>,
    /// Step C.3 — port over the Executioner's dynamic cdylib Tool registry.
    /// `Some` when the composition layer (C.5/C.6) has wired a
    /// `DynamicToolLoader`-backed impl; `None` leaves the `host.<name>`
    /// dispatch fallthrough dormant (built-in tools still resolve via the
    /// static match). Passed to `execute_orchestrator` each turn.
    dynamic_tools: Option<Arc<dyn crate::executor::DynamicToolPort>>,
    /// Step C.4.5.17 — port over the composition system (the IBS). `Some` when
    /// the composition layer has wired a `PgCompositionPort`-backed impl; `None`
    /// leaves the `host.compose_orchestrator` handler dormant (degrades
    /// gracefully to `{ok:false, error:"composition_unavailable"}`). Passed to
    /// `execute_orchestrator` each turn.
    composition_port: Option<Arc<dyn crate::executor::CompositionPort>>,
    /// DB-backed max wall-clock budget override for the Monty orchestrator VM.
    /// `Some` overrides `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` (Step 9.3).
    /// `None` falls back to the env-var / compiled-in DB-less default.
    max_duration_secs: Option<u64>,
}

impl ExecutionLoop {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        thread: Thread,
        llm: Arc<dyn LlmBackend>,
        effects: Arc<dyn EffectExecutor>,
        leases: Arc<LeaseManager>,
        policy: Arc<PolicyEngine>,
        signal_rx: SignalReceiver,
        user_id: String,
        gate_controller: Arc<dyn crate::gate::GateController>,
    ) -> Self {
        Self {
            thread,
            llm,
            effects,
            leases,
            policy,
            signal_rx,
            _user_id: user_id,
            capabilities: None,
            event_tx: None,
            retrieval: None,
            store: None,
            platform_info: None,
            gate_controller,
            #[cfg(feature = "skills-db")]
            pg_pool: None,
            retrieval_source: None,
            dynamic_tools: None,
            composition_port: None,
            max_duration_secs: None,
        }
    }

    /// Set the event broadcast sender for live status updates.
    pub fn with_event_tx(
        mut self,
        tx: tokio::sync::broadcast::Sender<crate::types::event::ThreadEvent>,
    ) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Set the capability registry for resolving capability-level policies.
    pub fn with_capabilities(
        mut self,
        capabilities: Arc<crate::capability::registry::CapabilityRegistry>,
    ) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Set the retrieval engine for injecting prior knowledge into context.
    pub fn with_retrieval(mut self, retrieval: crate::memory::RetrievalEngine) -> Self {
        self.retrieval = Some(retrieval);
        self
    }

    /// Set the Store for runtime prompt overlay loading and skill retrieval.
    pub fn with_store(mut self, store: Arc<dyn crate::traits::store::Store>) -> Self {
        self.store = Some(store);
        self
    }

    /// Set a Postgres pool for DB-backed skill loading (`skills-db` feature).
    #[cfg(feature = "skills-db")]
    pub fn with_pg_pool(mut self, pool: std::sync::Arc<brassclaw_pg::PgPool>) -> Self {
        self.pg_pool = Some(pool);
        self
    }

    /// Set platform metadata for self-awareness in system prompts.
    pub fn with_platform_info(mut self, info: crate::executor::prompt::PlatformInfo) -> Self {
        self.platform_info = Some(info);
        self
    }

    /// Set the Phase 5 retrieval source. v3 Phase H8.4 retired the
    /// `__assemble_prior_knowledge__` consumer; see the `retrieval_source` field
    /// doc for the current dormant-plumbing status.
    pub fn with_retrieval_source(
        mut self,
        source: Arc<dyn crate::memory::RetrievalSource>,
    ) -> Self {
        self.retrieval_source = Some(source);
        self
    }

    /// Step C.3 — attach the Executioner's dynamic cdylib Tool port. The impl
    /// (composition, C.5/C.6) delegates to `DynamicToolLoader`; the engine
    /// orchestrator's `host.<name>` dispatch fallthrough routes unknown calls
    /// through it. Without this the fallthrough is dormant.
    pub fn with_dynamic_tools(
        mut self,
        port: Arc<dyn crate::executor::DynamicToolPort>,
    ) -> Self {
        self.dynamic_tools = Some(port);
        self
    }

    /// Step C.4.5.17 — attach the composition-system port (the IBS). The impl
    /// (composition) backs `host.compose_orchestrator`: recipe fetch → IBS
    /// `build_instruction` → `compose_program` → `ComposedProgram`. Without this
    /// the host-call degrades gracefully.
    pub fn with_composition_port(
        mut self,
        port: Arc<dyn crate::executor::CompositionPort>,
    ) -> Self {
        self.composition_port = Some(port);
        self
    }

    /// Override the Monty orchestrator VM wall-clock budget with a value
    /// loaded from `reborn_monty_vm_settings.max_duration_secs` (Step 9.3).
    /// When set, takes priority over `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS`.
    pub fn with_max_duration_secs(mut self, secs: u64) -> Self {
        self.max_duration_secs = Some(secs);
        self
    }

    /// Add an event to the thread and broadcast it for live status updates.
    fn emit_event(&mut self, kind: EventKind) {
        let event = crate::types::event::ThreadEvent::new(self.thread.id, kind);
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event.clone());
        }
        self.thread.events.push(event);
        self.thread.updated_at = chrono::Utc::now();
    }

    fn load_runtime_checkpoint(&self) -> RuntimeCheckpoint {
        let persisted_state = self
            .thread
            .metadata
            .get(RUNTIME_CHECKPOINT_METADATA_KEY)
            .and_then(|value| value.get("persisted_state"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        RuntimeCheckpoint { persisted_state }
    }

    fn clear_runtime_checkpoint(&mut self) {
        if let Some(metadata) = self.thread.metadata.as_object_mut() {
            metadata.remove(RUNTIME_CHECKPOINT_METADATA_KEY);
        }
        self.thread.updated_at = chrono::Utc::now();
    }

    fn store_runtime_checkpoint(&mut self, checkpoint: &RuntimeCheckpoint) {
        if let Some(metadata) = self.thread.metadata.as_object_mut() {
            metadata.insert(
                RUNTIME_CHECKPOINT_METADATA_KEY.into(),
                serde_json::json!({
                    "persisted_state": checkpoint.persisted_state.clone(),
                }),
            );
        }
        self.thread.updated_at = chrono::Utc::now();
    }

    fn has_engine_owned_system_prompt(&self, checkpoint: &RuntimeCheckpoint) -> bool {
        let thread_has_prompt = |messages: &[crate::types::message::ThreadMessage]| {
            messages.iter().any(|message| {
                message.role == crate::types::message::MessageRole::System
                    && crate::executor::prompt::is_codeact_system_prompt(&message.content)
            })
        };

        thread_has_prompt(&self.thread.messages)
            || thread_has_prompt(&self.thread.internal_messages)
            || checkpoint.has_working_messages_system_prompt()
    }

    async fn refresh_system_prompt(
        &mut self,
        system_docs: &[crate::types::memory::MemoryDoc],
        system_docs_loaded: bool,
        checkpoint: &mut RuntimeCheckpoint,
    ) {
        let active_leases = self.leases.active_for_thread(self.thread.id).await;
        let prompt_context = crate::executor::thread_context::thread_execution_context(
            &self.thread,
            StepId::new(),
            None,
            self.gate_controller.clone(),
        );
        let capabilities_result = self
            .effects
            .available_capabilities(&active_leases, &prompt_context)
            .await;
        let capabilities_loaded = capabilities_result.is_ok();
        let capabilities = match capabilities_result {
            Ok(capabilities) => capabilities,
            Err(error) => {
                debug!(
                    thread_id = %self.thread.id,
                    "failed to load capabilities for system prompt refresh: {error}"
                );
                Vec::new()
            }
        };
        let actions_result = self
            .effects
            .available_actions(&active_leases, &prompt_context)
            .await;
        let actions_loaded = actions_result.is_ok();
        let compact_actions = match actions_result {
            Ok(actions) => actions,
            Err(error) => {
                debug!(
                    thread_id = %self.thread.id,
                    "failed to load actions for system prompt refresh: {error}"
                );
                Vec::new()
            }
        };
        if (!system_docs_loaded || !capabilities_loaded || !actions_loaded)
            && self.has_engine_owned_system_prompt(checkpoint)
        {
            debug!(
                thread_id = %self.thread.id,
                system_docs_loaded,
                capabilities_loaded,
                actions_loaded,
                "skipping system prompt refresh because prompt inputs are incomplete"
            );
            return;
        }
        let system_prompt = crate::executor::prompt::build_codeact_system_prompt_with_docs(
            &capabilities,
            &compact_actions,
            system_docs,
            self.platform_info.as_ref(),
        );

        let messages_updated = crate::executor::prompt::upsert_codeact_system_prompt(
            &mut self.thread.messages,
            system_prompt.clone(),
        );
        let internal_updated = if self.thread.internal_messages.is_empty() {
            false
        } else {
            crate::executor::prompt::upsert_codeact_system_prompt(
                &mut self.thread.internal_messages,
                system_prompt.clone(),
            )
        };
        let checkpoint_updated = checkpoint.update_working_messages_system_prompt(&system_prompt);

        if checkpoint_updated {
            self.store_runtime_checkpoint(checkpoint);
        } else if messages_updated || internal_updated {
            self.thread.updated_at = chrono::Utc::now();
        }
    }

    async fn persist_runtime_state(
        &self,
        step: Option<&Step>,
        persisted_event_count: &mut usize,
    ) -> Result<(), EngineError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };

        // All three store writes are independent — run them in parallel.
        let step_fut = async {
            if let Some(step) = step {
                store.save_step(step).await
            } else {
                Ok(())
            }
        };

        let new_event_count = self.thread.events.len();
        let events_fut = async {
            if *persisted_event_count < new_event_count {
                store
                    .append_events(&self.thread.events[*persisted_event_count..])
                    .await
            } else {
                Ok(())
            }
        };

        let thread_fut = store.save_thread(&self.thread);

        let (step_res, events_res, thread_res) = tokio::join!(step_fut, events_fut, thread_fut);
        step_res?;
        events_res?;
        thread_res?;

        *persisted_event_count = new_event_count;
        Ok(())
    }

    /// Run the execution loop to completion.
    pub async fn run(&mut self) -> Result<ThreadOutcome, EngineError> {
        let mut persisted_event_count = self.thread.events.len();
        let mut checkpoint = self.load_runtime_checkpoint();

        // Transition to Running if this is a fresh start or restart from a resumable state.
        if self.thread.state != ThreadState::Running {
            self.thread.transition_to(ThreadState::Running, None)?;
        }

        // Pre-fetch shared memory docs once — used by both prompt overlay and
        // orchestrator loading, avoiding a duplicate Store query.
        let (system_docs, system_docs_loaded) = if let Some(store) = self.store.as_ref() {
            match store.list_shared_memory_docs(self.thread.project_id).await {
                Ok(docs) => (docs, true),
                Err(e) => {
                    debug!("failed to load shared docs for orchestrator: {e}");
                    (Vec::new(), false)
                }
            }
        } else {
            (Vec::new(), true)
        };

        self.refresh_system_prompt(&system_docs, system_docs_loaded, &mut checkpoint)
            .await;
        self.persist_runtime_state(None, &mut persisted_event_count)
            .await?;

        // Load versioned Python orchestrator using pre-fetched docs.
        // Self-modification is disabled by default — only the compiled-in v0
        // runs unless explicitly opted in via ORCHESTRATOR_SELF_MODIFY=true.
        // The flag is read from the process-wide snapshot (set once on first
        // call) so a runtime env mutation cannot flip the gate mid-task.
        let allow_self_modify = crate::runtime::self_modify_enabled();
        let (orchestrator_code, orchestrator_version) =
            crate::executor::orchestrator::load_orchestrator_from_docs(
                &system_docs,
                allow_self_modify,
            );

        debug!(
            thread_id = %self.thread.id,
            orchestrator_version,
            "running Python orchestrator"
        );

        // Store version in thread metadata for rollback tracking
        if let Some(metadata) = self.thread.metadata.as_object_mut() {
            metadata.insert(
                "orchestrator_version".into(),
                serde_json::json!(orchestrator_version),
            );
        }

        // Execute the Python orchestrator with host function dispatch.
        // max_duration_override resolves the DB-backed budget (Step 9.3):
        // Some(dur) → uses DB value; None → falls back to env-var / compiled-in default.
        let max_duration_override = self.max_duration_secs.map(std::time::Duration::from_secs);
        let result = crate::executor::orchestrator::execute_orchestrator(
            &orchestrator_code,
            &mut self.thread,
            &self.llm,
            &self.effects,
            &self.leases,
            &self.policy,
            &mut self.signal_rx,
            self.event_tx.as_ref(),
            self.retrieval.as_ref(),
            self.store.as_ref(),
            self.platform_info.as_ref(),
            &self.gate_controller,
            &checkpoint.persisted_state,
            #[cfg(feature = "skills-db")]
            self.pg_pool.as_deref(),
            self.retrieval_source.as_ref(),
            self.dynamic_tools.as_ref(),
            self.composition_port.as_ref(),
            max_duration_override,
        )
        .await;

        // Post-cleanup: persist final state, track failures for auto-rollback
        match result {
            Ok(orch_result) => {
                // Reset failure counter on success
                if let Some(store) = self.store.as_ref() {
                    crate::executor::orchestrator::reset_orchestrator_failures(
                        store,
                        self.thread.project_id,
                    )
                    .await;
                }
                let _ = &orch_result.tokens_used;

                self.clear_runtime_checkpoint();
                self.persist_runtime_state(None, &mut persisted_event_count)
                    .await?;
                Ok(orch_result.outcome)
            }
            Err(e) => {
                debug!(
                    thread_id = %self.thread.id,
                    error = %e,
                    orchestrator_version,
                    "orchestrator execution failed"
                );

                // Record failure for auto-rollback tracking
                if let Some(store) = self.store.as_ref() {
                    crate::executor::orchestrator::record_orchestrator_failure(
                        store,
                        self.thread.project_id,
                        orchestrator_version,
                    )
                    .await;

                    // Emit rollback event if this version will be skipped next time
                    // (failure count was just incremented, so check >= threshold - 1)
                    if orchestrator_version > 0 {
                        self.emit_event(EventKind::OrchestratorRollback {
                            from_version: orchestrator_version,
                            to_version: orchestrator_version.saturating_sub(1),
                            reason: format!("execution failed: {e}"),
                        });
                    }
                }

                // Transition to failed if not already in a terminal state
                if self.thread.state != ThreadState::Completed
                    && self.thread.state != ThreadState::Failed
                    && self.thread.state != ThreadState::Done
                {
                    let _ = self.thread.transition_to(
                        ThreadState::Failed,
                        Some(format!("orchestrator error: {e}")),
                    );
                }
                self.clear_runtime_checkpoint();
                self.persist_runtime_state(None, &mut persisted_event_count)
                    .await?;
                Ok(ThreadOutcome::Failed {
                    error: format!("Orchestrator error: {e}"),
                    debug_detail: e.debug_detail().map(|s| s.to_string()),
                })
            }
        }
    }
}
