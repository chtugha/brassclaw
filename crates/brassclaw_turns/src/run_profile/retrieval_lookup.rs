//! Intent-driven retrieval lookup port.
//!
//! Defined at the loop layer (`brassclaw_turns`) so that:
//! 1. `brassclaw_agent_loop::executor::recipe_stage` can call it through the
//!    existing `AgentLoopDriverHost` (via [`crate::run_profile::LoopRetrievalPort`])
//!    without `brassclaw_agent_loop` or `brassclaw_reborn` gaining a direct
//!    dependency on `brassclaw_engine` types.
//! 2. The composition layer (`brassclaw_reborn_composition`) — the sole crate
//!    depending on both `brassclaw_engine` and the agent-loop stack — provides
//!    the implementation backed by `brassclaw_engine::memory::PostgresSource`.
//!
//! This mirrors the established `RecipeLookup` / `LoopRecipePort` crate-boundary
//! discipline: the trait + DTOs live here (turns-native, no engine types), the
//! engine-backed impl lives in composition, and the production host
//! (`RebornLoopDriverHost`) holds an `Option<Arc<dyn RetrievalLookup>>` threaded
//! in via a builder. See `docs/agents-v3/subplan_problem_stepE0_of_saved_plan_to_v3.md`.
//!
//! The trait surface is deliberately narrow — only the single
//! `fetch_for_turn` method the recipe pipeline actually calls.

use std::fmt;

use async_trait::async_trait;

use crate::run_profile::LoopRunContext;

/// Result of an intent-driven `fetch_for_turn` call, surfaced to the agent loop.
///
/// This is a **`brassclaw_turns`-native** type: it carries the routing signals
/// and pre-serialized component arrays as `serde_json::Value` so that neither
/// `brassclaw_agent_loop` nor `brassclaw_reborn` must depend on
/// `brassclaw_engine`'s `ComponentItem` / `FetchForTurnResult`. The composition
/// `RetrievalLookup` impl performs the `ComponentItem -> serde_json::Value`
/// serialization at the crate boundary.
///
/// Field meanings (per the v3 plan §H4):
/// - `tier0_eligible`: true when the matched recipe is mature/candidate +
///   wilson_lower >= 0.70 + validated + validation hook wired. Full Tier-0
///   eligibility check. **Conservative default `false` until Phase E's
///   `SplitResult` populates it for real** (E0-A scope).
/// - `llm_call_required`: true when the recipe declares `llm_call_required =
///   true`. Tier 0 requires both `tier0_eligible == true` AND
///   `llm_call_required == false`. **Conservative default `true` until Phase E**
///   (so Tier-0 short-circuit never fires before the consumer lands in Phase H).
/// - `rust_items`: serialized `Vec<ComponentItem>` for the rust channel
///   (ToolSkills, PythonCode helpers). Applied to the Rust execution context
///   before Python starts. **E0-A: identical to `orchestrator_items` (unsplit);
///   Phase E's `SplitResult` performs the real rust/orchestrator split.**
/// - `orchestrator_items`: serialized `Vec<ComponentItem>` for the orchestrator
///   channel (Skills, PythonCode). Stashed in `state.last_retrieval_result`
///   (E0-A) / `state.recipe_hint` (Phase H) for the Python step-0 handler.
/// - `routing_meta`: routing metadata (variant label, matched component UUID
///   count, etc.) for telemetry and stash/unstash disambiguation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetrievalTurnResult {
    pub tier0_eligible: bool,
    pub llm_call_required: bool,
    pub rust_items: serde_json::Value,
    pub orchestrator_items: serde_json::Value,
    pub routing_meta: serde_json::Value,
}

/// Async errors raised by retrieval lookups. Callers should treat every
/// variant as fatal — the lookup returns `Ok(None)` on a soft miss.
///
/// Mirrors [`crate::run_profile::RecipeLookupError`]'s shape (manual
/// `Display`/`Error` impls, no `thiserror` dependency in this module).
#[derive(Debug)]
pub enum RetrievalLookupError {
    Db(String),
    Decode(String),
    Backend(String),
}

impl fmt::Display for RetrievalLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(reason) => write!(f, "retrieval lookup db error: {reason}"),
            Self::Decode(reason) => write!(f, "retrieval lookup decode error: {reason}"),
            Self::Backend(reason) => write!(f, "retrieval lookup backend error: {reason}"),
        }
    }
}

impl std::error::Error for RetrievalLookupError {}

/// Intent-driven retrieval lookup contract.
///
/// All methods are `async` — the backing store is typically a DB and must not
/// be driven via `block_on()` inside a running Tokio runtime (deadlock risk on
/// single-threaded or work-stealing executors). Mirrors
/// [`crate::run_profile::RecipeLookup`].
///
/// Returns `Ok(None)` when no component matches (soft miss — caller falls
/// through to Tier 2). Returns `Ok(Some(_))` with either assembled components
/// (serialized into `RetrievalTurnResult`) or a disambiguation payload. Returns
/// `Err(_)` on a hard backend failure.
#[async_trait]
pub trait RetrievalLookup: Send + Sync {
    /// Intent-driven retrieval for a live turn (v3 plan §H4 / §6.7).
    ///
    /// `sender_class_code` is the numeric class-code prefix of the calling
    /// component (e.g. `"02"` for the orchestrator). The composition impl
    /// delegates to `PostgresSource::fetch_for_turn`, which runs
    /// `resolve_intent` and either fetches the specific component by ID,
    /// surfaces disambiguation candidates, or falls back to the keyword path.
    async fn fetch_for_turn(
        &self,
        context: &LoopRunContext,
        query: &str,
        token_budget: usize,
        sender_class_code: &str,
    ) -> Result<Option<RetrievalTurnResult>, RetrievalLookupError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_profile::{AgentLoopDriverHost, LoopRetrievalPort, NoRetrieval};

    #[test]
    fn retrieval_turn_result_serde_round_trip() {
        let result = RetrievalTurnResult {
            tier0_eligible: false,
            llm_call_required: true,
            rust_items: serde_json::json!([{ "id": "02-001" }]),
            orchestrator_items: serde_json::json!([{ "id": "04-001" }]),
            routing_meta: serde_json::json!({ "variant": "orchestrator", "count": 2 }),
        };
        let encoded = serde_json::to_string(&result).expect("serialize");
        let decoded: RetrievalTurnResult = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded.tier0_eligible, result.tier0_eligible);
        assert_eq!(decoded.llm_call_required, result.llm_call_required);
        assert_eq!(decoded.rust_items, result.rust_items);
        assert_eq!(decoded.orchestrator_items, result.orchestrator_items);
        assert_eq!(decoded.routing_meta, result.routing_meta);
    }

    #[test]
    fn no_retrieval_returns_none() {
        assert!(NoRetrieval.retrieval_lookup().is_none());
    }

    // Compile-time proof that `LoopRetrievalPort` is a supertrait of
    // `AgentLoopDriverHost`: the method is reachable on any driver host
    // without importing the port trait separately. Body is checked against
    // the `H: AgentLoopDriverHost` bound at definition time.
    fn _retrieval_port_reachable_via_driver_host<H>(host: &H)
    where
        H: AgentLoopDriverHost + ?Sized,
    {
        let _ = host.retrieval_lookup();
    }

    // Stub `RetrievalLookup` returning a conservative (E0-A) result, plus a
    // minimal host exposing it. These are test doubles for the port-wiring
    // contract; the production impl lives in composition (Step 3). The real
    // `fetch_for_turn` invocation is exercised by the Step 3/4 composition +
    // RecipeStage tests (which own a live `LoopRunContext`); here we only
    // prove the trait is implementable and the host port surfaces it.
    struct StubRetrieval;

    #[async_trait]
    impl RetrievalLookup for StubRetrieval {
        async fn fetch_for_turn(
            &self,
            _context: &LoopRunContext,
            _query: &str,
            _token_budget: usize,
            _sender_class_code: &str,
        ) -> Result<Option<RetrievalTurnResult>, RetrievalLookupError> {
            Ok(Some(RetrievalTurnResult {
                tier0_eligible: false,
                llm_call_required: true,
                rust_items: serde_json::json!([]),
                orchestrator_items: serde_json::json!([]),
                routing_meta: serde_json::json!({ "stub": true }),
            }))
        }
    }

    struct StubRetrievalHost {
        lookup: StubRetrieval,
    }

    impl LoopRetrievalPort for StubRetrievalHost {
        fn retrieval_lookup(&self) -> Option<&dyn RetrievalLookup> {
            Some(&self.lookup)
        }
    }

    #[test]
    fn stub_host_exposes_retrieval_lookup() {
        let host = StubRetrievalHost {
            lookup: StubRetrieval,
        };
        assert!(host.retrieval_lookup().is_some());
    }
}
