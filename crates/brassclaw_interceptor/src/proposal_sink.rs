//! `SempaiProposalSink` — submission port for Sempai-proposed components.
//!
//! When the Sempai reviews a Kohai prompt (rerouting mode) it may include
//! `proposed_recipe_updates` and `proposed_intent_examples` in its
//! [`crate::SempaiReviewOutcome`].  These proposed blobs must enter the
//! Q1 validation queue (`validation_status = 'pending'`, `queue_code =
//! 'q1_auto'`) rather than being written directly to production tables.
//!
//! This trait is the submission port.  Composition implements it via
//! `PgSempaiProposalSink` (backed by `PgRecipeStore` insert); a no-op
//! implementation is provided for builds where no store is wired.

use async_trait::async_trait;

use crate::error::InterceptorError;

/// Result of a [`SempaiProposalSink::submit_proposals`] call.
#[derive(Debug, Clone)]
pub struct ProposalSubmitResult {
    /// Number of recipe/skill update proposals successfully enqueued in Q1.
    pub recipe_updates_queued: u32,
    /// Number of intent-example proposals successfully enqueued in Q1.
    pub intent_examples_queued: u32,
}

/// Submission port for Sempai-proposed component updates and intent examples.
///
/// Implementations route the raw JSON payloads produced by the Sempai into
/// the appropriate Q1 validation tables.  The sink operates on best-effort
/// semantics — failures are logged and counted but do **not** abort the
/// interceptor pipeline (the Kohai call still proceeds with the adjusted
/// messages even if proposal submission fails).
#[async_trait]
pub trait SempaiProposalSink: Send + Sync {
    /// Submit Sempai-proposed component updates and intent examples to Q1.
    ///
    /// - `user_id` / `project_id` — the scope that owns the submitted rows.
    /// - `proposed_recipe_updates` — raw JSON blobs; each is expected to
    ///   contain at minimum a `"name"` field and either a `"steps"` field
    ///   (recipe) or a `"tool_name"` field (skill).  Missing or malformed
    ///   entries are skipped and counted as errors; remaining valid entries
    ///   are still submitted.
    /// - `proposed_intent_examples` — raw JSON blobs with at minimum an
    ///   `"input"` field (the example text) and an optional `"class"` field
    ///   (intent class 1–4; defaults to 1).
    ///
    /// Returns counts of successfully queued items.
    async fn submit_proposals(
        &self,
        user_id: &str,
        project_id: &str,
        proposed_recipe_updates: &[serde_json::Value],
        proposed_intent_examples: &[serde_json::Value],
    ) -> Result<ProposalSubmitResult, InterceptorError>;
}

/// A [`SempaiProposalSink`] that silently discards all proposals.
///
/// Used when no recipe store is wired (non-postgres builds or when the
/// store is not yet available on the current path).
pub struct NoopProposalSink;

#[async_trait]
impl SempaiProposalSink for NoopProposalSink {
    async fn submit_proposals(
        &self,
        _user_id: &str,
        _project_id: &str,
        _proposed_recipe_updates: &[serde_json::Value],
        _proposed_intent_examples: &[serde_json::Value],
    ) -> Result<ProposalSubmitResult, InterceptorError> {
        Ok(ProposalSubmitResult {
            recipe_updates_queued: 0,
            intent_examples_queued: 0,
        })
    }
}
