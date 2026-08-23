//! Orchestrator bridge lookup port — Tier-0/Tier-1 prior-knowledge + reply DTOs.
//!
//! Defined at the loop layer (`brassclaw_turns`) so that:
//! 1. `brassclaw_agent_loop::executor::recipe_stage` / `canonical.rs` can call the
//!    orchestrator bridge through the existing `AgentLoopDriverHost` (via
//!    [`crate::run_profile::LoopOrchestratorPort`]) without `brassclaw_agent_loop`
//!    or `brassclaw_reborn` gaining a direct dependency on `brassclaw_engine` types.
//! 2. The composition layer (`brassclaw_reborn_composition`) — the sole crate
//!    depending on both `brassclaw_engine` and the agent-loop stack — provides the
//!    implementation backed by the engine `pub` orchestrator fns extracted in
//!    v3 Phase H.8 (`assemble_prior_knowledge_with_hint` + `execute_tier_zero_channel`).
//!
//! This mirrors the established `RetrievalLookup` / `LoopRetrievalPort` and
//! `RecipeLookup` / `LoopRecipePort` crate-boundary discipline: the DTOs (+ the
//! `OrchestratorLookup` trait added in v3 Phase H.7) live here (turns-native, no
//! engine types), the engine-backed impl lives in composition, and the production
//! host (`RebornLoopDriverHost`) holds an `Option<Arc<dyn OrchestratorLookup>>`
//! threaded in via a builder. See `docs/agents-v3/subplan_problem_stepH_of_saved_plan_to_v3.md`.
//!
//! **Reused by Model B/C (v3 Phase H.5 O4):** these types are consumed solely by
//! the Model B/C agent-loop Tier-0/Tier-1 path — the `LoopOrchestratorPort`
//! driver plus the engine `pub` fns extracted in H.8. The Model A `default.py`
//! step-0 `tier_zero` branch that previously embodied this logic was removed in
//! v3 Phase H.5 O3 (Model A is dormant/never-built — production turns run on the
//! agent loop, not the engine Python runtime).
//!
//! This module defines the two DTOs the H.6 plan item requires
//! ([`PriorKnowledgeBundle`] + [`TierZeroReply`]) plus the [`OrchestratorLookup`]
//! trait (v3 Phase H.7). The `LoopOrchestratorPort` accessor + `NoOrchestrator`
//! default port impl live in [`crate::run_profile::host`] alongside the other
//! ports (mirroring `LoopRetrievalPort` / `NoRetrieval`).

use async_trait::async_trait;

use crate::run_profile::LoopRunContext;

/// Returned by `LoopOrchestratorPort::run_step_zero` (v3 Phase H.7). Carries the
/// formatted prior-knowledge bundle that `PromptStage` / the composition
/// `build_prompt_bundle` injects into the LLM prompt for Tier 1. Plain string +
/// metadata — no engine types.
///
/// This is a **`brassclaw_turns`-native** type: the composition `OrchestratorLookup`
/// impl performs the engine → `String` / `Vec<String>` serialization at the crate
/// boundary so neither `brassclaw_agent_loop` nor `brassclaw_reborn` must depend on
/// `brassclaw_engine`'s orchestrator types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriorKnowledgeBundle {
    /// The assembled `orchestrator_content` block (Skills + PythonCode bodies,
    /// formatted). This is what `PromptStage` prepends to the LLM context window
    /// for a Tier-1 turn.
    pub orchestrator_content: String,
    /// The UUIDs of matched components, for telemetry and `record_recipe_outcome`.
    pub matched_component_ids: Vec<String>,
    /// When true, the composition host chose to replace the entire prompt with
    /// this content (Solution Override path, §3.13). Normally false — Tier 1
    /// prepends the bundle alongside the normal prompt.
    pub override_prompt_creation: bool,
}

/// Returned by `LoopOrchestratorPort::run_tier_zero` (v3 Phase H.7). The Tier-0
/// reply text to emit directly to the user, with no LLM call.
///
/// Same crate-boundary discipline as [`PriorKnowledgeBundle`]: turns-native,
/// serialized at the composition crate boundary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TierZeroReply {
    /// The formatted output text to emit as the assistant reply.
    pub text: String,
    /// The UUIDs of matched components that produced this reply, for Wilson
    /// scoring (`record_recipe_outcome`).
    pub matched_component_ids: Vec<String>,
}

/// Bridge from agent-loop stages to the engine orchestrator (v3 Phase H.7).
///
/// All methods are `async` — the backing engine fns
/// (`assemble_prior_knowledge_with_hint` / `execute_tier_zero_channel`,
/// extracted in v3 Phase H.8) drive the Monty VM and must not be driven via
/// `block_on()` inside a running Tokio runtime (deadlock risk on single-threaded
/// or work-stealing executors). Mirrors [`crate::run_profile::RetrievalLookup`].
///
/// Returns `None` (not `Err`) when there is nothing to assemble or no Tier-0
/// channel to run — the caller degrades to Tier 2 (Tier-0) or skips the
/// prior-knowledge prepend (Tier-1). Hard engine failures are logged inside the
/// composition impl and surfaced as `None` so a recipe-channel failure never
/// aborts the turn (degrade-gracefully, mirroring the engine
/// `RecipeTierZeroFailed` → Tier-2 degradation).
///
/// The composition layer is the sole implementor (it depends on both
/// `brassclaw_engine` and the agent-loop stack). Production hosts hold an
/// `Option<Arc<dyn OrchestratorLookup>>` threaded in via a builder; hosts
/// without an orchestrator bridge inherit [`crate::run_profile::NoOrchestrator`]
/// (the `LoopOrchestratorPort` accessor returns `None`).
#[async_trait]
pub trait OrchestratorLookup: Send + Sync {
    /// Tier 1: run Python step-0 prior-knowledge assembly. Reads the stashed
    /// `recipe_hint` (one-shot consume — `None` means no stash / already
    /// consumed) and returns the formatted prior-knowledge bundle that
    /// `PromptStage` / `build_prompt_bundle` injects into the LLM prompt. Does
    /// NOT call the LLM. `None` when no orchestrator bridge is wired or assembly
    /// produced nothing.
    async fn run_step_zero(
        &self,
        context: &LoopRunContext,
        recipe_hint: Option<&serde_json::Value>,
    ) -> Option<PriorKnowledgeBundle>;

    /// Tier 0: run the orchestrator channel (skills + PythonCode) with NO LLM.
    /// Consumes the stashed `recipe_hint` (orchestrator_items) +
    /// `recipe_rust_context` and returns the reply text for
    /// `AssistantReplyStage`. `None` when no orchestrator bridge is wired or the
    /// channel produced no reply — `RecipeStage` then falls back to Tier 2.
    async fn run_tier_zero(
        &self,
        context: &LoopRunContext,
        recipe_hint: &serde_json::Value,
        recipe_rust_context: &serde_json::Value,
    ) -> Option<TierZeroReply>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_profile::{AgentLoopDriverHost, LoopOrchestratorPort, NoOrchestrator};

    #[test]
    fn prior_knowledge_bundle_serde_round_trip() {
        let bundle = PriorKnowledgeBundle {
            orchestrator_content: "## [skill: greet]\nbody\n".to_string(),
            matched_component_ids: vec!["04-001".to_string(), "22-002".to_string()],
            override_prompt_creation: false,
        };
        let encoded = serde_json::to_string(&bundle).expect("serialize");
        let decoded: PriorKnowledgeBundle = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded.orchestrator_content, bundle.orchestrator_content);
        assert_eq!(decoded.matched_component_ids, bundle.matched_component_ids);
        assert_eq!(
            decoded.override_prompt_creation,
            bundle.override_prompt_creation
        );
    }

    #[test]
    fn tier_zero_reply_serde_round_trip() {
        let reply = TierZeroReply {
            text: "Hello — greeting skill fired.".to_string(),
            matched_component_ids: vec!["04-001".to_string()],
        };
        let encoded = serde_json::to_string(&reply).expect("serialize");
        let decoded: TierZeroReply = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded.text, reply.text);
        assert_eq!(decoded.matched_component_ids, reply.matched_component_ids);
    }

    #[test]
    fn no_orchestrator_returns_none() {
        assert!(NoOrchestrator.orchestrator_lookup().is_none());
    }

    // Compile-time proof that `LoopOrchestratorPort` is a supertrait of
    // `AgentLoopDriverHost`: the accessor is reachable on any driver host
    // without importing the port trait separately.
    fn _orchestrator_port_reachable_via_driver_host<H>(host: &H)
    where
        H: AgentLoopDriverHost + ?Sized,
    {
        let _ = host.orchestrator_lookup();
    }

    // Stub `OrchestratorLookup` returning fixed bundle/reply payloads, plus a
    // minimal host exposing it. Test doubles for the port-wiring contract; the
    // production impl lives in composition (v3 Phase H.12).
    struct StubOrchestrator;

    #[async_trait]
    impl OrchestratorLookup for StubOrchestrator {
        async fn run_step_zero(
            &self,
            _context: &LoopRunContext,
            recipe_hint: Option<&serde_json::Value>,
        ) -> Option<PriorKnowledgeBundle> {
            Some(PriorKnowledgeBundle {
                orchestrator_content: "## [skill: greet]\nbody\n".to_string(),
                matched_component_ids: recipe_hint
                    .and_then(|v| v.get("matched_component_ids"))
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                override_prompt_creation: false,
            })
        }

        async fn run_tier_zero(
            &self,
            _context: &LoopRunContext,
            _recipe_hint: &serde_json::Value,
            _recipe_rust_context: &serde_json::Value,
        ) -> Option<TierZeroReply> {
            Some(TierZeroReply {
                text: "FINAL(stub tier-zero reply)".to_string(),
                matched_component_ids: vec!["04-001".to_string()],
            })
        }
    }

    struct StubOrchestratorHost {
        lookup: StubOrchestrator,
    }

    impl LoopOrchestratorPort for StubOrchestratorHost {
        fn orchestrator_lookup(&self) -> Option<&dyn OrchestratorLookup> {
            Some(&self.lookup)
        }
    }

    #[test]
    fn stub_host_exposes_orchestrator_lookup() {
        let host = StubOrchestratorHost {
            lookup: StubOrchestrator,
        };
        assert!(host.orchestrator_lookup().is_some());
    }
}
