//! Production [`OrchestratorLookup`] bridge (v3 Phase H.12.5).
//!
//! [`PgOrchestratorLookup`] is the composition-layer impl of the
//! turns-native [`OrchestratorLookup`] trait — the sole implementor, mirroring
//! [`PgRetrievalLookup`] in `retrieval_lookup_impl.rs`. It bridges the agent
//! loop's Tier-0/Tier-1 stages to the engine deterministic-execution channel:
//!
//! - **Tier 1** (`run_step_zero`): loads the live engine [`Thread`] from
//!   `brassclaw_session_threads` (via [`PgThreadEngineStore`], H.12.5.1),
//!   then calls [`TierZeroOrchestrator::assemble_prior_knowledge`] with the
//!   stashed `recipe_hint` (`Some`-branch — no fresh fetch, no LLM) and maps
//!   the engine [`PkrAssemblyResult`] → turns-native [`PriorKnowledgeBundle`]
//!   for `PromptStage` / `build_prompt_bundle` to prepend (H.12.6).
//! - **Tier 0** (`run_tier_zero`): loads the [`Thread`], builds the per-run
//!   [`EffectExecutor`] via [`TierZeroEffectExecutorBuilder::build_for_run`]
//!   (Q-H12-2-BUILD = A — tenant/agent/grants resolve per turn), then calls
//!   [`TierZeroOrchestrator::run_tier_zero`] (deterministic, NO LLM) and maps
//!   [`TierZeroChannelResult`] → [`TierZeroReply`] for `AssistantReplyStage`.
//!
//! Both methods return `None` (not `Err`) on a thread-load miss or any engine
//! error — degrade-gracefully so a recipe-channel failure never aborts the
//! turn, mirroring the engine `RecipeTierZeroFailed` → Tier-2 degradation and
//! the [`PgRetrievalLookup`] error mapping.
//!
//! Gated behind the composition `skills-db` feature: the held
//! [`TierZeroEffectExecutorBuilder`] only exists under `skills-db`, so the
//! type + its trait impl are `#[cfg(feature = "skills-db")]`. Under the default
//! feature set the host's `orchestrator_lookup` slot stays `None` →
//! `NoOrchestrator` → Tier-2 degrade. The pure mapping helpers compile under
//! both configs (they touch only always-available engine types) and are covered
//! by unit tests that run under both; `#![allow(dead_code)]` covers the
//! unused-under-default window, mirroring `orchestrator_effect_executor.rs` /
//! `pg_thread_engine_store.rs`.

#![allow(dead_code)]
#![forbid(unsafe_code)]

#[cfg(feature = "skills-db")]
use std::sync::Arc;

#[cfg(feature = "skills-db")]
use async_trait::async_trait;
#[cfg(feature = "skills-db")]
use uuid::Uuid;

use brassclaw_engine::executor::{PkrAssemblyResult, TierZeroChannelResult};
#[cfg(feature = "skills-db")]
use brassclaw_engine::types::thread::{Thread, ThreadId as EngineThreadId};
#[cfg(feature = "skills-db")]
use brassclaw_engine::{Store, TierZeroOrchestrator};
#[cfg(feature = "skills-db")]
use brassclaw_turns::run_profile::{LoopRunContext, OrchestratorLookup};
use brassclaw_turns::run_profile::{PriorKnowledgeBundle, TierZeroReply};

#[cfg(feature = "skills-db")]
use crate::runtime::TierZeroEffectExecutorBuilder;

/// `sender_class_code` for the orchestrator channel (class 02) — the calling
/// component that drives intent-driven retrieval in the recipe stage. Mirrors
/// `brassclaw_agent_loop::executor::recipe::RECIPE_SENDER_CLASS_CODE` (private
/// there); the orchestrator channel IS class 02, so the value is identical.
const ORCHESTRATOR_SENDER_CLASS_CODE: &str = "02";

/// Token budget for Tier-1 prior-knowledge assembly. Mirrors
/// `brassclaw_agent_loop::executor::recipe::RETRIEVAL_TOKEN_BUDGET` (private
/// there): the assembled prior-knowledge block gets the same headroom the
/// retrieval stage grants a fresh `fetch_for_turn`.
const PRIOR_KNOWLEDGE_TOKEN_BUDGET: usize = 4096;

/// Production [`OrchestratorLookup`] backed by the engine Tier-0 channel.
///
/// Holds three long-lived deps constructed once at runtime wiring time:
/// - `runtime` — the [`TierZeroOrchestrator`] facade (engine deterministic
///   channel; `LlmBackend` is the always-erroring [`TierZeroLlmGuard`] so a
///   mis-compiled recipe surfaces loudly instead of silently calling a model).
/// - `thread_store` — the PG-backed engine [`Store`] ([`PgThreadEngineStore`]),
///   the canonical loader for the live [`Thread`] Tier-0/Tier-1 needs.
/// - `executor_builder` — the per-run [`EffectExecutor`] factory
///   ([`TierZeroEffectExecutorBuilder`], built in `capability_wiring` from the
///   same host runtime + policy + mounts the capability port factory uses).
///
/// The per-run [`EffectExecutor`] is intentionally NOT held here — it is built
/// per turn by `build_for_run` so tenant/agent/grants resolve at run time.
#[cfg(feature = "skills-db")]
pub(crate) struct PgOrchestratorLookup {
    runtime: Arc<TierZeroOrchestrator>,
    thread_store: Arc<dyn Store>,
    executor_builder: Arc<TierZeroEffectExecutorBuilder>,
}

#[cfg(feature = "skills-db")]
impl PgOrchestratorLookup {
    pub(crate) fn new(
        runtime: Arc<TierZeroOrchestrator>,
        thread_store: Arc<dyn Store>,
        executor_builder: Arc<TierZeroEffectExecutorBuilder>,
    ) -> Self {
        Self {
            runtime,
            thread_store,
            executor_builder,
        }
    }

    /// Load the live engine [`Thread`] for `context.thread_id`, mapping the
    /// turns `brassclaw_host_api::ThreadId` → engine `ThreadId(pub Uuid)`.
    /// Returns `None` on a parse failure, a store miss, or a store error —
    /// every miss shape degrades gracefully (the caller skips the channel).
    async fn load_thread(&self, context: &LoopRunContext) -> Option<Thread> {
        let uuid = Uuid::parse_str(context.thread_id.as_str()).ok()?;
        match self.thread_store.load_thread(EngineThreadId(uuid)).await {
            Ok(Some(thread)) => Some(thread),
            Ok(None) => None,
            Err(error) => {
                tracing::debug!(
                    %error,
                    "PgOrchestratorLookup::load_thread failed; degrading to None"
                );
                None
            }
        }
    }
}

#[cfg(feature = "skills-db")]
#[async_trait]
impl OrchestratorLookup for PgOrchestratorLookup {
    async fn run_step_zero(
        &self,
        context: &LoopRunContext,
        recipe_hint: Option<&serde_json::Value>,
    ) -> Option<PriorKnowledgeBundle> {
        let thread = self.load_thread(context).await?;
        // Tier-1 `recipe_hint` is `Some` (RecipeStage stashed the
        // orchestrator-channel items) → the engine `Some`-branch assembles them
        // with NO second `fetch_for_turn` and NO LLM call. `goal` is both the
        // execution prompt and the implicit assembly query.
        let pkr = match self
            .runtime
            .assemble_prior_knowledge(
                &thread,
                &thread.goal,
                PRIOR_KNOWLEDGE_TOKEN_BUDGET,
                ORCHESTRATOR_SENDER_CLASS_CODE,
                recipe_hint.cloned(),
            )
            .await
        {
            Ok(pkr) => pkr,
            Err(error) => {
                tracing::debug!(
                    %error,
                    "PgOrchestratorLookup::run_step_zero assemble failed; degrading to None"
                );
                return None;
            }
        };
        Some(map_pkr_to_bundle(pkr))
    }

    async fn run_tier_zero(
        &self,
        context: &LoopRunContext,
        recipe_hint: &serde_json::Value,
        recipe_rust_context: &serde_json::Value,
    ) -> Option<TierZeroReply> {
        let thread = self.load_thread(context).await?;
        let effects = match self.executor_builder.build_for_run(context).await {
            Ok(effects) => effects,
            Err(error) => {
                tracing::debug!(
                    %error,
                    "PgOrchestratorLookup::run_tier_zero build_for_run failed; degrading to None"
                );
                return None;
            }
        };
        let result = match self
            .runtime
            .run_tier_zero(&thread, &effects, recipe_hint, recipe_rust_context)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::debug!(
                    %error,
                    "PgOrchestratorLookup::run_tier_zero channel failed; degrading to None"
                );
                return None;
            }
        };
        Some(map_result_to_reply(result))
    }
}

/// Map the engine [`PkrAssemblyResult`] → turns-native [`PriorKnowledgeBundle`].
///
/// Drops the vestigial Q2 fields (`action_short_circuit` / `disambiguation` /
/// `candidates` / `tier_zero`) — those route via `RetrievalTurnResult` and are
/// branched on in `RecipeStage` before `run_step_zero` is reached, so they are
/// never populated for the Tier-1 assemble-only path. The three carried fields
/// are exactly what `PromptStage` / `build_prompt_bundle` consumes (H.12.6).
fn map_pkr_to_bundle(pkr: PkrAssemblyResult) -> PriorKnowledgeBundle {
    PriorKnowledgeBundle {
        orchestrator_content: pkr.orchestrator_content,
        matched_component_ids: pkr.matched_component_ids,
        override_prompt_creation: pkr.override_prompt_creation,
    }
}

/// Map the engine [`TierZeroChannelResult`] → turns-native [`TierZeroReply`].
///
/// `formatted_output` (the last successful PythonCode step's reply text) →
/// `text` (emitted as the assistant reply); `matched_component_ids` carry for
/// Wilson scoring (`record_recipe_outcome`).
fn map_result_to_reply(result: TierZeroChannelResult) -> TierZeroReply {
    TierZeroReply {
        text: result.formatted_output,
        matched_component_ids: result.matched_component_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_pkr_to_bundle_preserves_core_fields_and_drops_vestigial() {
        let pkr = PkrAssemblyResult {
            orchestrator_content: "## [Skill: foo]\nbody".to_string(),
            matched_component_ids: vec!["uuid-1".to_string(), "uuid-2".to_string()],
            override_prompt_creation: true,
            // Vestigial Q2 fields — must be dropped by the mapping.
            action_short_circuit: true,
            action_component_id: Some("action-id".to_string()),
            action_name: Some("action-name".to_string()),
            disambiguation: true,
            candidates: vec![serde_json::json!({"component_id": "x"})],
            tier_zero: true,
        };
        let bundle = map_pkr_to_bundle(pkr);
        assert_eq!(bundle.orchestrator_content, "## [Skill: foo]\nbody");
        assert_eq!(bundle.matched_component_ids, vec!["uuid-1", "uuid-2"]);
        assert!(bundle.override_prompt_creation);
    }

    #[test]
    fn map_result_to_reply_renames_formatted_output_to_text() {
        let result = TierZeroChannelResult {
            formatted_output: "the deterministic reply".to_string(),
            matched_component_ids: vec!["uuid-3".to_string()],
        };
        let reply = map_result_to_reply(result);
        assert_eq!(reply.text, "the deterministic reply");
        assert_eq!(reply.matched_component_ids, vec!["uuid-3"]);
    }
}
