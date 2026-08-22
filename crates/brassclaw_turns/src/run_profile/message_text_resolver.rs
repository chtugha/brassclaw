//! Raw user-message text resolution for live agent-loop turns (v3 plan §H3).
//!
//! Defined at the loop layer (`brassclaw_turns`) so the production host
//! (`RebornLoopDriverHost`) can expose the raw accepted-message body to the
//! recipe pipeline (intent matching) without `brassclaw_reborn` gaining a
//! dependency on `brassclaw_first_party_extension_ports` (which owns the
//! `messages_by_run` store). The composition layer — the sole crate depending
//! on both — supplies the implementation backed by
//! `SelectableSkillContextSource::peek_message_text`.
//!
//! This mirrors the `RetrievalLookup` / `RecipeLookup` crate-boundary
//! discipline: the trait lives here (turns-native, returns `Option<String>`),
//! the `messages_by_run`-backed impl lives in composition, and the production
//! host holds an `Option<Arc<dyn MessageTextResolver>>` threaded in via a
//! builder. See `docs/agents-v3/subplan_problem_stepE0_of_saved_plan_to_v3.md`.
//!
//! The trait surface is deliberately narrow — only the single
//! `resolve_message_text` method the InputStage / RecipeStage pipeline calls.

use async_trait::async_trait;

use crate::LoopMessageRef;
use crate::run_profile::{AgentLoopHostError, LoopRunContext};

/// Raw accepted-message text resolver for a live turn (v3 plan §H3).
///
/// Returns the **raw** accepted-message body (`Some(text)`) or `None` when no
/// message is recorded for the ref (already taken by the consuming
/// activation path or never written). The raw text — NOT the sanitized
/// `safe_summary` — is required so intent matching is not corrupted by
/// `[redacted]` placeholders (plan §H3, `saved_plan_to_v3.md:5387–5390`).
///
/// The host's [`crate::run_profile::LoopContextPort::resolve_message_text`]
/// default returns `Err(Unimplemented)`; `RebornLoopDriverHost` overrides it
/// to delegate to a wired `MessageTextResolver`, mapping `Ok(Some(text))` →
/// `Ok(text)`, `Ok(None)` / unwired → `Err(Unimplemented)` (Tier-2
/// fall-through). Callers (`InputStage::drain`) populate
/// `state.last_user_text` on `Ok` and leave it `None` on `Err`.
#[async_trait]
pub trait MessageTextResolver: Send + Sync {
    /// Resolve the raw accepted-message body for `message_ref` in `context`.
    ///
    /// - `Ok(Some(text))` — raw message body recorded for this ref.
    /// - `Ok(None)` — no message recorded (soft miss; caller falls back).
    /// - `Err(_)` — hard backend failure (e.g. poisoned lock).
    async fn resolve_message_text(
        &self,
        context: &LoopRunContext,
        message_ref: &LoopMessageRef,
    ) -> Result<Option<String>, AgentLoopHostError>;
}
