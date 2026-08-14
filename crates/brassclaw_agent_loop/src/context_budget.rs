//! Per-turn context budget distribution.
//!
//! [`TurnContextBudget`] is built once per turn from the active model's registered
//! `context_window_tokens` and then threaded to every consumer (skill selector,
//! context strategy, skill context service). Slices are **advisory starting-point
//! hints** — any consumer that exhausts its slice can absorb headroom left by
//! others as long as the aggregate stays under `model_context_window`.
//!
//! The only hard invariant: no single consumer may cause the total sent to the
//! model to exceed `model_context_window - output_reserved`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// BudgetAllocation
// ---------------------------------------------------------------------------

/// Advisory allocation percentages for a [`TurnContextBudget`].
///
/// These are **starting-point hints**, not hard per-category limits. Any consumer
/// that exhausts its slice can absorb headroom from others as long as the aggregate
/// stays under `model_context_window`. Operators who want more room for everything
/// simply increase `context_window_tokens` in the provider config or switch to a
/// larger model — no special flag or override mechanism exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetAllocation {
    /// Fraction of the context window held back for the model's own reply output.
    pub output_reserved_pct: u8,
    /// Starting advisory share of the remaining window for skill snippets.
    pub skill_snippet_pct: u8,
    /// Starting advisory share of the remaining window for tool output text.
    pub tool_output_pct: u8,
    // history_pct is implied: 100 - skill_snippet_pct - tool_output_pct
}

impl Default for BudgetAllocation {
    fn default() -> Self {
        Self {
            output_reserved_pct: 15,
            skill_snippet_pct: 20,
            tool_output_pct: 15,
        }
    }
}

// ---------------------------------------------------------------------------
// TurnContextBudget
// ---------------------------------------------------------------------------

/// Pre-computed, per-turn advisory budget derived from the active model's context window.
///
/// Slices are **soft defaults**, not per-category hard limits. The only hard constraint
/// is `model_context_window`. Consumers receive their starting allocation; they report
/// actual usage back so the next consumer can absorb any remaining headroom.
///
/// Built via [`TurnContextBudget::from_context_window`] (uses [`BudgetAllocation::default`])
/// or [`TurnContextBudget::from_context_window_with_allocation`] for operator-tuned fractions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnContextBudget {
    /// Total context window in tokens. Hard ceiling — never exceeded.
    pub model_context_window: u32,
    /// Tokens held back for the model's own reply output. Always reserved.
    pub output_reserved: u32,
    /// Starting advisory allocation for system instructions + skill snippets.
    pub skill_snippet_tokens: u32,
    /// Starting advisory allocation for conversation history (messages).
    pub history_tokens: u32,
    /// Starting advisory allocation for tool output text.
    pub tool_output_tokens: u32,
}

impl TurnContextBudget {
    /// Minimum tokens always held back for the model's reply, regardless of percentage.
    pub const MIN_OUTPUT_RESERVED: u32 = 4_096;

    /// Build from a resolved model context window using default allocation fractions.
    ///
    /// Default fractions (see [`BudgetAllocation::default`]):
    ///   - output_reserved : 15 % of window (floor: 4 096, ceiling: window)
    ///   - skill_snippets  : 20 % of remaining
    ///   - tool_output     : 15 % of remaining
    ///   - history         : remaining ~65 %
    pub fn from_context_window(context_window_tokens: u32) -> Self {
        Self::from_context_window_with_allocation(
            context_window_tokens,
            &BudgetAllocation::default(),
        )
    }

    /// Build from a resolved model context window with explicit allocation fractions.
    pub fn from_context_window_with_allocation(
        context_window_tokens: u32,
        allocation: &BudgetAllocation,
    ) -> Self {
        let output_reserved =
            ((context_window_tokens as u64 * allocation.output_reserved_pct as u64) / 100) as u32;
        // Floor at MIN_OUTPUT_RESERVED tokens so small-window models still have room for a reply.
        let output_reserved = output_reserved
            .max(Self::MIN_OUTPUT_RESERVED)
            .min(context_window_tokens);
        let remaining = context_window_tokens.saturating_sub(output_reserved);
        let skill_snippet_tokens =
            (remaining as u64 * allocation.skill_snippet_pct as u64 / 100) as u32;
        let tool_output_tokens =
            (remaining as u64 * allocation.tool_output_pct as u64 / 100) as u32;
        let history_tokens = remaining
            .saturating_sub(skill_snippet_tokens)
            .saturating_sub(tool_output_tokens);
        Self {
            model_context_window: context_window_tokens,
            output_reserved,
            skill_snippet_tokens,
            history_tokens,
            tool_output_tokens,
        }
    }

    /// Bytes equivalent of `skill_snippet_tokens` (4 chars ≈ 1 token).
    pub fn skill_snippet_bytes(&self) -> usize {
        self.skill_snippet_tokens as usize * 4
    }

    /// Bytes equivalent of `history_tokens` (4 chars ≈ 1 token).
    pub fn history_bytes(&self) -> usize {
        self.history_tokens as usize * 4
    }

    /// Bytes equivalent of `tool_output_tokens` (4 chars ≈ 1 token).
    pub fn tool_output_bytes(&self) -> usize {
        self.tool_output_tokens as usize * 4
    }

    /// Estimated maximum messages that fit in `history_tokens`.
    ///
    /// `avg_tokens_per_message` should come from [`ObservedMessageAverage::get_tokens`].
    pub fn max_messages_estimate(&self, avg_tokens_per_message: u32) -> u32 {
        self.history_tokens / avg_tokens_per_message.max(1)
    }
}

/// Default fallback context window when the provider's value is not registered.
///
/// Mirrors [`brassclaw_llm::registry::FALLBACK_CONTEXT_WINDOW`] — defined here
/// so `brassclaw_agent_loop` consumers don't need to depend on `brassclaw_llm`.
pub const DEFAULT_FALLBACK_CONTEXT_WINDOW: u32 = 16_000;

// ---------------------------------------------------------------------------
// ObservedMessageAverage
// ---------------------------------------------------------------------------

/// Rolling exponential moving average of observed tokens per message.
///
/// Updated after each turn by the executor with
/// `actual_prompt_tokens / messages_in_bundle` from the model usage response.
/// Thread-safe; shared via `Arc` across the loop. EMA α = 0.25, favouring recent data.
///
/// Replaces the hard-coded `AVG_TOKENS_PER_MESSAGE = 200` constant.
#[derive(Debug, Clone)]
pub struct ObservedMessageAverage {
    /// EMA stored as fixed-point (tokens × 100) for lock-free integer arithmetic.
    value: Arc<AtomicUsize>,
}

impl ObservedMessageAverage {
    /// Starting estimate — replaced quickly once real usage data flows in.
    pub const DEFAULT_AVG_TOKENS_PER_MESSAGE: u32 = 300;
    /// EMA old-value weight (out of 100). Retains 75% of the previous estimate.
    const EMA_OLD_WEIGHT: u64 = 75;
    /// EMA new-value weight (out of 100). Incorporates 25% of the new observation (α = 0.25).
    const EMA_NEW_WEIGHT: u64 = 25;

    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicUsize::new(
                Self::DEFAULT_AVG_TOKENS_PER_MESSAGE as usize * 100,
            )),
        }
    }

    /// Read current estimate in whole tokens.
    pub fn get_tokens(&self) -> u32 {
        (self.value.load(Ordering::Relaxed) / 100) as u32
    }

    /// Update with a new observed per-message average (EMA α = 0.25).
    ///
    /// Call after each turn with `usage.prompt_tokens / messages_in_bundle`.
    /// No-op when `observed_tokens_per_message` is zero.
    pub fn update(&self, observed_tokens_per_message: u32) {
        if observed_tokens_per_message == 0 {
            return;
        }
        let prev = self.value.load(Ordering::Relaxed) as u64;
        // EMA fixed-point: weight 75% old, 25% new (α = 0.25), scaled by 100.
        let next = (prev * Self::EMA_OLD_WEIGHT
            + (observed_tokens_per_message as u64 * 100) * Self::EMA_NEW_WEIGHT)
            / 100;
        self.value.store(next as usize, Ordering::Relaxed);
    }
}

impl Default for ObservedMessageAverage {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Representative model context-window sizes used across tests.
    const CTX_8K: u32 = 8_000;
    const CTX_16K: u32 = 16_000;
    const CTX_32K: u32 = 32_000;
    const CTX_128K: u32 = 128_000;
    const CTX_200K: u32 = 200_000;
    const CTX_1M: u32 = 1_000_000;

    fn slices_sum_to_window(budget: &TurnContextBudget) {
        assert_eq!(
            budget.output_reserved
                + budget.skill_snippet_tokens
                + budget.history_tokens
                + budget.tool_output_tokens,
            budget.model_context_window,
            "slices must sum to model_context_window for window={}",
            budget.model_context_window
        );
    }

    #[test]
    fn budget_slices_sum_to_window_8k() {
        let b = TurnContextBudget::from_context_window(CTX_8K);
        slices_sum_to_window(&b);
        assert!(b.history_tokens > 0);
        assert!(b.output_reserved >= TurnContextBudget::MIN_OUTPUT_RESERVED);
    }

    #[test]
    fn budget_slices_sum_to_window_16k() {
        let b = TurnContextBudget::from_context_window(CTX_16K);
        slices_sum_to_window(&b);
        assert!(b.history_tokens > 0);
        assert!(b.output_reserved >= TurnContextBudget::MIN_OUTPUT_RESERVED);
    }

    #[test]
    fn budget_slices_sum_to_window_32k() {
        let b = TurnContextBudget::from_context_window(CTX_32K);
        slices_sum_to_window(&b);
        assert!(b.history_tokens > 0);
        assert!(b.output_reserved >= TurnContextBudget::MIN_OUTPUT_RESERVED);
    }

    #[test]
    fn budget_slices_sum_to_window_128k() {
        let b = TurnContextBudget::from_context_window(CTX_128K);
        slices_sum_to_window(&b);
        assert!(b.history_tokens > 0);
        assert!(b.output_reserved >= TurnContextBudget::MIN_OUTPUT_RESERVED);
    }

    #[test]
    fn budget_slices_sum_to_window_200k() {
        let b = TurnContextBudget::from_context_window(CTX_200K);
        slices_sum_to_window(&b);
        assert!(b.history_tokens > 0);
        assert!(b.output_reserved >= TurnContextBudget::MIN_OUTPUT_RESERVED);
    }

    #[test]
    fn budget_slices_sum_to_window_1m() {
        let b = TurnContextBudget::from_context_window(CTX_1M);
        slices_sum_to_window(&b);
        assert!(b.history_tokens > 0);
        assert!(b.output_reserved >= TurnContextBudget::MIN_OUTPUT_RESERVED);
    }

    #[test]
    fn observed_message_average_starts_at_default() {
        let avg = ObservedMessageAverage::new();
        assert_eq!(
            avg.get_tokens(),
            ObservedMessageAverage::DEFAULT_AVG_TOKENS_PER_MESSAGE
        );
    }

    #[test]
    fn observed_message_average_updates_toward_observed() {
        let avg = ObservedMessageAverage::new();
        // After many updates of 100 tokens, should converge toward 100.
        for _ in 0..40 {
            avg.update(100);
        }
        assert!(
            avg.get_tokens() < 200,
            "should have converged toward 100, got {}",
            avg.get_tokens()
        );
    }

    #[test]
    fn observed_message_average_ignores_zero() {
        let avg = ObservedMessageAverage::new();
        let before = avg.get_tokens();
        avg.update(0);
        assert_eq!(avg.get_tokens(), before);
    }

    #[test]
    fn custom_allocation_sums_to_window() {
        let alloc = BudgetAllocation {
            output_reserved_pct: 10,
            skill_snippet_pct: 30,
            tool_output_pct: 10,
        };
        let b = TurnContextBudget::from_context_window_with_allocation(CTX_128K, &alloc);
        slices_sum_to_window(&b);
    }
}
