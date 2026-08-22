//! Tier-0 recipe-outcome event listener (v3 Phase H4.7, Q-H7-surface A2).
//!
//! Subscribes to the engine `ThreadEvent` broadcast and, on a terminal
//! [`EventKind::RecipeTierZeroSucceeded`] /
//! [`EventKind::RecipeTierZeroFailed`] event, calls
//! [`RecipeLookup::record_recipe_outcome`] `(recipe_id, success)` — the
//! atomic Wilson-update on the matched Recipe. Fire-and-forget
//! best-effort: recording errors are logged at `debug!` and never
//! break the projection task or the turn.
//!
//! **Q-H7 Architecture A:** the Wilson-update transaction stays in
//! composition `PgRecipeLibrary` — the engine never depends on
//! `brassclaw_turns`. This listener is the composition-side bridge
//! between the engine `event_tx` broadcast and the `RecipeLookup`
//! store.
//!
//! **Dormant-runtime-ready (Option A):** the engine `ThreadManager`
//! runtime that emits on `event_tx` is dormant in the live agent-loop
//! path today (`execute_orchestrator` is only called from
//! `loop_engine.rs:471`; the live driver is the agent-loop stack whose
//! `TurnEventSink` only sees turn-lifecycle events). So this listener
//! is built + unit tested here and wired into the live event stream
//! when Phase H.6–H.13 route Tier-0 execution through the engine pub
//! fns that emit on `event_tx`. The listener logic itself is fully
//! implemented (no stub): `handle_event` is a pure async fn over a
//! `&ThreadEvent` and is unit-tested directly with a spy
//! `RecipeLookup`.

use std::sync::Arc;

use brassclaw_engine::types::event::{EventKind, ThreadEvent};
use brassclaw_turns::run_profile::RecipeLookup;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Composition-side listener that records Tier-0 recipe outcomes on the
/// matched Recipe via [`RecipeLookup::record_recipe_outcome`].
///
/// Construct with an `Arc<dyn RecipeLookup>` (typically the same
/// `PgRecipeLibrary` the runtime already wires for `RecipeStage`) and
/// either call [`handle_event`] directly per event or [`spawn`] the
/// broadcast-draining projection task.
pub struct RecipeOutcomeListener {
    recipe_lookup: Arc<dyn RecipeLookup>,
}

impl std::fmt::Debug for RecipeOutcomeListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `dyn RecipeLookup` has no `Debug` supertrait, so derive can't
        // be used; print a stable placeholder instead.
        f.debug_struct("RecipeOutcomeListener")
            .field("recipe_lookup", &"<dyn RecipeLookup>")
            .finish()
    }
}

impl RecipeOutcomeListener {
    /// Wrap a [`RecipeLookup`] handle. The handle is held by `Arc` so it
    /// can be shared with the spawned projection task.
    pub fn new(recipe_lookup: Arc<dyn RecipeLookup>) -> Self {
        Self { recipe_lookup }
    }

    /// Process a single engine [`ThreadEvent`].
    ///
    /// On a terminal `RecipeTierZeroSucceeded` / `RecipeTierZeroFailed`
    /// event carrying a non-empty `recipe_id`, calls
    /// `record_recipe_outcome(recipe_id, success)`. Best-effort: a
    /// recording failure is logged at `debug!` and swallowed — a
    /// Wilson-update failure must never break the projection task or
    /// the turn (the outcome is also carried on
    /// `OrchestratorResult.tier_zero_outcome` for engine-internal
    /// consumers, per H4.6). Non-terminal events (e.g.
    /// `RecipeTierZeroStarted`) and empty-`recipe_id` events are
    /// ignored.
    pub async fn handle_event(&self, event: &ThreadEvent) {
        let (recipe_id, success) = match &event.kind {
            EventKind::RecipeTierZeroSucceeded { recipe_id, .. } => (recipe_id.clone(), true),
            EventKind::RecipeTierZeroFailed { recipe_id, .. } => (recipe_id.clone(), false),
            _ => return,
        };
        if recipe_id.is_empty() {
            tracing::debug!(
                success,
                "recipe_outcome_listener: terminal tier-zero event with empty recipe_id, skipping"
            );
            return;
        }
        if let Err(error) = self
            .recipe_lookup
            .record_recipe_outcome(&recipe_id, success)
            .await
        {
            tracing::debug!(
                recipe_id = recipe_id.as_str(),
                success,
                error = %error,
                "recipe_outcome_listener: record_recipe_outcome failed (best-effort)"
            );
        }
    }

    /// Spawn the projection task that drains `receiver` and hands every
    /// event to [`handle_event`]. The returned
    /// [`RecipeOutcomeProjection`] cancels + awaits the task on
    /// shutdown (mirror of `BudgetEventProjection`).
    pub fn spawn(self, receiver: broadcast::Receiver<ThreadEvent>) -> RecipeOutcomeProjection {
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        let handle = tokio::spawn(run_projection(
            receiver,
            self.recipe_lookup,
            cancel_for_task,
        ));
        RecipeOutcomeProjection { handle, cancel }
    }
}

/// Handle to the spawned recipe-outcome projection task. Cancelled +
/// awaited via [`shutdown`](Self::shutdown); holding it keeps the task
/// alive, dropping it without shutdown detaches the task (it continues
/// until the broadcast closes).
#[derive(Debug)]
pub struct RecipeOutcomeProjection {
    handle: JoinHandle<()>,
    cancel: CancellationToken,
}

impl RecipeOutcomeProjection {
    /// Trigger cancellation and await the projection task. Idempotent
    /// — safe to call after the task has already exited.
    pub async fn shutdown(self) {
        self.cancel.cancel();
        if let Err(error) = self.handle.await {
            if error.is_panic() {
                tracing::error!(
                    %error,
                    "recipe outcome projection task panicked during shutdown"
                );
            } else {
                tracing::debug!(
                    %error,
                    "recipe outcome projection task cancelled during shutdown"
                );
            }
        }
    }
}

async fn run_projection(
    mut receiver: broadcast::Receiver<ThreadEvent>,
    recipe_lookup: Arc<dyn RecipeLookup>,
    cancel: CancellationToken,
) {
    let listener = RecipeOutcomeListener { recipe_lookup };
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::debug!("recipe outcome projection cancelled — exiting");
                return;
            }
            received = receiver.recv() => {
                match received {
                    Ok(event) => listener.handle_event(&event).await,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            "recipe outcome projection fell behind the broadcast buffer; \
                             dropping {skipped} events and resuming"
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::debug!(
                            "recipe outcome event broadcast closed — projection exiting"
                        );
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use brassclaw_engine::types::thread::ThreadId;
    use brassclaw_turns::run_profile::{RecipeLookupError, RecipeMatchDto, ToolSkillMatchDto};
    use std::sync::Mutex;

    /// Spy `RecipeLookup` that captures every `record_recipe_outcome`
    /// call (recipe_id, success) in arrival order. The other trait
    /// methods are no-ops — they are not exercised by the listener.
    #[derive(Debug, Default)]
    struct SpyRecipeLookup {
        calls: Mutex<Vec<(String, bool)>>,
    }

    #[async_trait]
    impl RecipeLookup for SpyRecipeLookup {
        async fn find_recipe(
            &self,
            _user_input: &str,
        ) -> Result<Option<RecipeMatchDto>, RecipeLookupError> {
            Ok(None)
        }
        async fn find_skills(
            &self,
            _user_input: &str,
        ) -> Result<Vec<ToolSkillMatchDto>, RecipeLookupError> {
            Ok(Vec::new())
        }
        async fn record_recipe_outcome(
            &self,
            recipe_id: &str,
            success: bool,
        ) -> Result<(), RecipeLookupError> {
            self.calls
                .lock()
                .expect("spy calls mutex not poisoned")
                .push((recipe_id.to_string(), success));
            Ok(())
        }
        async fn record_skill_outcome(
            &self,
            _skill_id: &str,
            _success: bool,
        ) -> Result<(), RecipeLookupError> {
            Ok(())
        }
    }

    fn make_event(kind: EventKind) -> ThreadEvent {
        ThreadEvent::new(ThreadId::new(), kind)
    }

    #[tokio::test]
    async fn handle_event_records_outcome_for_succeeded_and_failed_only() {
        // v3 Phase H4.7 (Q-H7-surface A2): the listener must call
        // `record_recipe_outcome(recipe_id, success)` for the TERMINAL
        // `RecipeTierZeroSucceeded` (success=true) + `RecipeTierZeroFailed`
        // (success=false) events, and must NOT call it for the
        // non-terminal `RecipeTierZeroStarted` event.
        let spy = Arc::new(SpyRecipeLookup::default());
        let recipe_lookup: Arc<dyn RecipeLookup> = spy.clone();
        let listener = RecipeOutcomeListener::new(recipe_lookup);

        let ok_id = "11111111-1111-1111-1111-111111111111";
        let bad_id = "22222222-2222-2222-2222-222222222222";

        // started — non-terminal, must be ignored.
        listener
            .handle_event(&make_event(EventKind::RecipeTierZeroStarted {
                recipe_id: ok_id.to_string(),
                recipe_name: "greet-recipe".to_string(),
            }))
            .await;
        // succeeded — terminal success.
        listener
            .handle_event(&make_event(EventKind::RecipeTierZeroSucceeded {
                recipe_id: ok_id.to_string(),
                recipe_name: "greet-recipe".to_string(),
            }))
            .await;
        // failed — terminal failure.
        listener
            .handle_event(&make_event(EventKind::RecipeTierZeroFailed {
                recipe_id: bad_id.to_string(),
                recipe_name: "bad-recipe".to_string(),
                message: "step raised: boom".to_string(),
            }))
            .await;

        let calls = spy.calls.lock().expect("spy calls mutex not poisoned");
        assert_eq!(
            *calls,
            vec![(ok_id.to_string(), true), (bad_id.to_string(), false),],
            "only terminal events record an outcome, in arrival order"
        );
    }

    #[tokio::test]
    async fn handle_event_skips_empty_recipe_id() {
        // A terminal event with an empty recipe_id (e.g. a non-recipe
        // path where routing.recipe_id was None → serialized null →
        // extract_string_kwarg.unwrap_or_default() → "") must NOT
        // trigger a recording — recording an outcome against an empty
        // id would be a no-op DB call at best.
        let spy = Arc::new(SpyRecipeLookup::default());
        let recipe_lookup: Arc<dyn RecipeLookup> = spy.clone();
        let listener = RecipeOutcomeListener::new(recipe_lookup);

        listener
            .handle_event(&make_event(EventKind::RecipeTierZeroSucceeded {
                recipe_id: String::new(),
                recipe_name: "greet-recipe".to_string(),
            }))
            .await;

        let calls = spy.calls.lock().expect("spy calls mutex not poisoned");
        assert!(
            calls.is_empty(),
            "empty recipe_id must not trigger record_recipe_outcome"
        );
    }

    #[tokio::test]
    async fn handle_event_swallows_record_failure() {
        // Best-effort: a `record_recipe_outcome` failure must be
        // swallowed (logged at debug!) and never propagate — the
        // listener must keep processing subsequent events.
        #[derive(Debug, Default)]
        struct FailingRecipeLookup {
            calls: Mutex<usize>,
        }

        #[async_trait]
        impl RecipeLookup for FailingRecipeLookup {
            async fn find_recipe(
                &self,
                _: &str,
            ) -> Result<Option<RecipeMatchDto>, RecipeLookupError> {
                Ok(None)
            }
            async fn find_skills(
                &self,
                _: &str,
            ) -> Result<Vec<ToolSkillMatchDto>, RecipeLookupError> {
                Ok(Vec::new())
            }
            async fn record_recipe_outcome(
                &self,
                _recipe_id: &str,
                _success: bool,
            ) -> Result<(), RecipeLookupError> {
                *self.calls.lock().expect("failing spy lock") += 1;
                Err(RecipeLookupError::Backend("db unavailable".to_string()))
            }
            async fn record_skill_outcome(
                &self,
                _: &str,
                _: bool,
            ) -> Result<(), RecipeLookupError> {
                Ok(())
            }
        }

        let spy = Arc::new(FailingRecipeLookup::default());
        let recipe_lookup: Arc<dyn RecipeLookup> = spy.clone();
        let listener = RecipeOutcomeListener::new(recipe_lookup);

        // Must not panic / propagate the error.
        listener
            .handle_event(&make_event(EventKind::RecipeTierZeroSucceeded {
                recipe_id: "11111111-1111-1111-1111-111111111111".to_string(),
                recipe_name: "greet-recipe".to_string(),
            }))
            .await;

        assert_eq!(
            *spy.calls.lock().expect("failing spy lock"),
            1,
            "record_recipe_outcome must have been attempted once"
        );
    }
}
