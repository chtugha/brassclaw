//! Tier-0 [`LlmBackend`] guard: a real semantic guard, not a stub.
//!
//! Tier-0 recipes are deterministic — they never call `__llm_complete__`
//! (`execute_tier_zero_channel` only reaches the LLM through this backend).
//! If a recipe wrongly reaches for the LLM, [`TierZeroLlmGuard::complete`]
//! returns [`EngineError::InvalidInput`], which `execute_tier_zero_channel`
//! maps to a `TierZeroStep::Degrade` → Tier-2 fallback (per H.11). The guard
//! therefore surfaces mis-compiled recipes loudly instead of silently
//! executing an unintended model call. If a future Tier-0 recipe genuinely
//! needs an LLM, replace this with a model-gateway adapter (out of H.12
//! scope) — do not silently allow calls here.
//!
//! `dead_code` is allowed module-wide: the guard is only **constructed** under
//! the `skills-db` feature (H.12.4 wiring), so under the default feature set
//! the type is defined-but-unused. The type itself is feature-agnostic and its
//! unit test runs under both configs.

#![allow(dead_code)]

use brassclaw_engine::types::capability::ActionDef;
use brassclaw_engine::{EngineError, LlmBackend, LlmCallConfig, LlmOutput, ThreadMessage};

/// Semantic guard passed as the `llm` backend to `execute_tier_zero_channel`.
///
/// [`LlmBackend::complete`] always errors: the Tier-0 channel is deterministic.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TierZeroLlmGuard;

impl TierZeroLlmGuard {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl LlmBackend for TierZeroLlmGuard {
    async fn complete(
        &self,
        _messages: &[ThreadMessage],
        _actions: &[ActionDef],
        _config: &LlmCallConfig,
    ) -> Result<LlmOutput, EngineError> {
        Err(EngineError::InvalidInput {
            reason: "Tier-0 channel does not call the LLM".into(),
        })
    }

    fn model_name(&self) -> &str {
        "tier-zero-guard"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn complete_always_errors_with_semantic_reason() {
        let guard = TierZeroLlmGuard::new();
        let err = guard
            .complete(&[], &[], &LlmCallConfig::default())
            .await
            .expect_err("guard refuses all LLM calls");
        match err {
            EngineError::InvalidInput { reason } => {
                assert_eq!(reason, "Tier-0 channel does not call the LLM");
            }
            other => panic!("expected EngineError::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn model_name_identifies_the_guard() {
        assert_eq!(TierZeroLlmGuard::new().model_name(), "tier-zero-guard");
    }

    #[test]
    fn default_constructs_without_args() {
        let _guard: TierZeroLlmGuard = Default::default();
    }
}
