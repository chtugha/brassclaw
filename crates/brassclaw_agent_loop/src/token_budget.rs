//! Token budget estimation and tracking for prompt composition.
//!
//! Provides utilities to estimate token counts from text content and track
//! cumulative token usage during prompt assembly. Uses a simple heuristic
//! (~4 characters per token) that works reasonably well for English text.

/// Estimate the number of tokens in a text string.
///
/// Uses a simple heuristic: ~4 characters per token. This is a rough
/// approximation that works reasonably well for English prose and code.
/// More sophisticated tokenizers (like tiktoken) could be used for
/// better accuracy, but this is sufficient for budget planning.
///
/// # Examples
///
/// ```
/// use brassclaw_agent_loop::token_budget::estimate_tokens;
///
/// let text = "Hello, world!";
/// let tokens = estimate_tokens(text);
/// assert_eq!(tokens, 4); // (13 + 3) / 4 = 4
/// ```
pub fn estimate_tokens(content: &str) -> usize {
    // ~4 characters per token is a reasonable heuristic for English text.
    // This slightly underestimates for dense code and overestimates for
    // whitespace-heavy content, but averages out well in practice.
    content.len().saturating_add(3) / 4
}

/// Track cumulative token budget during message selection.
///
/// Helps enforce token limits during prompt assembly by tracking how many
/// tokens have been consumed and how many remain in the budget.
#[derive(Debug, Clone)]
pub struct TokenBudgetTracker {
    max_tokens: usize,
    used_tokens: usize,
}

impl TokenBudgetTracker {
    /// Create a new budget tracker with the specified maximum tokens.
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - Maximum number of tokens allowed in the budget
    ///
    /// # Examples
    ///
    /// ```
    /// use brassclaw_agent_loop::token_budget::TokenBudgetTracker;
    ///
    /// let tracker = TokenBudgetTracker::new(8000);
    /// assert_eq!(tracker.remaining(), 8000);
    /// ```
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
        }
    }

    /// Create a budget tracker with unlimited tokens.
    ///
    /// Useful when token budgeting is disabled or not applicable.
    pub fn unlimited() -> Self {
        Self {
            max_tokens: usize::MAX,
            used_tokens: 0,
        }
    }

    /// Check if adding the specified number of tokens would exceed the budget.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to check
    ///
    /// # Returns
    ///
    /// `true` if adding these tokens would exceed the budget, `false` otherwise
    pub fn would_exceed(&self, tokens: usize) -> bool {
        self.used_tokens.saturating_add(tokens) > self.max_tokens
    }

    /// Consume the specified number of tokens from the budget.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to consume
    ///
    /// # Returns
    ///
    /// `true` if the tokens were consumed successfully, `false` if the budget
    /// was exceeded (tokens are still consumed in this case)
    pub fn consume(&mut self, tokens: usize) -> bool {
        let would_exceed = self.would_exceed(tokens);
        self.used_tokens = self.used_tokens.saturating_add(tokens);
        !would_exceed
    }

    /// Get the number of tokens remaining in the budget.
    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Get the number of tokens used so far.
    pub fn used(&self) -> usize {
        self.used_tokens
    }

    /// Get the maximum token budget.
    pub fn max(&self) -> usize {
        self.max_tokens
    }

    /// Check if the budget has been exceeded.
    pub fn is_exceeded(&self) -> bool {
        self.used_tokens > self.max_tokens
    }

    /// Get the budget utilization as a percentage (0.0 to 1.0+).
    pub fn utilization(&self) -> f64 {
        if self.max_tokens == 0 {
            return 0.0;
        }
        self.used_tokens as f64 / self.max_tokens as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        // "Hello" = 5 chars -> (5 + 3) / 4 = 2 tokens
        assert_eq!(estimate_tokens("Hello"), 2);
    }

    #[test]
    fn test_estimate_tokens_medium() {
        // "Hello, world!" = 13 chars -> (13 + 3) / 4 = 4 tokens
        assert_eq!(estimate_tokens("Hello, world!"), 4);
    }

    #[test]
    fn test_estimate_tokens_long() {
        let text = "The quick brown fox jumps over the lazy dog";
        // 44 chars -> (44 + 3) / 4 = 11 tokens
        assert_eq!(estimate_tokens(text), 11);
    }

    #[test]
    fn test_budget_tracker_new() {
        let tracker = TokenBudgetTracker::new(100);
        assert_eq!(tracker.max(), 100);
        assert_eq!(tracker.used(), 0);
        assert_eq!(tracker.remaining(), 100);
        assert!(!tracker.is_exceeded());
    }

    #[test]
    fn test_budget_tracker_unlimited() {
        let tracker = TokenBudgetTracker::unlimited();
        assert_eq!(tracker.max(), usize::MAX);
        assert_eq!(tracker.remaining(), usize::MAX);
    }

    #[test]
    fn test_budget_tracker_consume() {
        let mut tracker = TokenBudgetTracker::new(100);

        assert!(tracker.consume(30));
        assert_eq!(tracker.used(), 30);
        assert_eq!(tracker.remaining(), 70);

        assert!(tracker.consume(50));
        assert_eq!(tracker.used(), 80);
        assert_eq!(tracker.remaining(), 20);

        assert!(!tracker.consume(30)); // Exceeds budget
        assert_eq!(tracker.used(), 110);
        assert!(tracker.is_exceeded());
    }

    #[test]
    fn test_budget_tracker_would_exceed() {
        let mut tracker = TokenBudgetTracker::new(100);
        tracker.consume(80);

        assert!(!tracker.would_exceed(20));
        assert!(tracker.would_exceed(21));
        assert!(tracker.would_exceed(100));
    }

    #[test]
    fn test_budget_tracker_utilization() {
        let mut tracker = TokenBudgetTracker::new(100);

        assert_eq!(tracker.utilization(), 0.0);

        tracker.consume(50);
        assert_eq!(tracker.utilization(), 0.5);

        tracker.consume(50);
        assert_eq!(tracker.utilization(), 1.0);

        tracker.consume(10);
        assert_eq!(tracker.utilization(), 1.1);
    }

    #[test]
    fn test_budget_tracker_zero_max() {
        let tracker = TokenBudgetTracker::new(0);
        assert_eq!(tracker.utilization(), 0.0);
        assert!(tracker.would_exceed(1));
    }
}
