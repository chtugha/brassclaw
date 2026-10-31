//! Configuration checks around the existing provider chain, never a model loop.
use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::{
    CompletionRequest, CompletionResponse, LlmError, LlmProvider, ModelMetadata,
    ModelRequestTokenCounts, ModelTokenCheckError, ModelTokenLimits, ResponseCachePolicy,
    ToolCompletionRequest, ToolCompletionResponse,
};

#[derive(Debug, Clone, Copy)]
pub struct ProviderRequestFacts<'a> {
    pub model: &'a str,
    pub max_output_tokens: Option<u32>,
}

/// One atomic live provider-read result. These are transient observations of
/// the existing definition, not another settings store or an authorization.
#[derive(Debug, Clone)]
pub struct ProviderDispatchState {
    pub definition_revision: u64,
    pub response_cache: ResponseCachePolicy,
    pub provider_id: String,
    pub model: String,
    pub model_token_limits: Option<ModelTokenLimits>,
    /// Declared context for this exact model only. This is not deployed geometry.
    pub configured_context_window_tokens: Option<u32>,
    /// The original request facts; never invent output headroom when absent.
    pub requested_output_tokens: Option<u32>,
}
impl ProviderDispatchState {
    /// Compare measured counts with this captured target's declared limits.
    /// The caller still must authenticate the actual prepared request/render,
    /// accepted prefix and deployment, and recheck this revision before send.
    /// Success here is arithmetic only, never a final dispatch permission.
    pub fn check_configured_token_counts(
        &self,
        provider_id: &str,
        model: &str,
        counts: ModelRequestTokenCounts,
    ) -> Result<(), ModelTokenCheckError> {
        if self.provider_id != provider_id || self.model != model {
            return Err(ModelTokenCheckError::TargetMismatch);
        }
        if self.definition_revision == 0 {
            return Err(ModelTokenCheckError::InvalidPolicy);
        }
        let limits = self
            .model_token_limits
            .ok_or(ModelTokenCheckError::MissingInputLimits)?;
        let context = self
            .configured_context_window_tokens
            .ok_or(ModelTokenCheckError::UnknownContext)?;
        let output = self
            .requested_output_tokens
            .ok_or(ModelTokenCheckError::UnknownOutputReservation)?;
        limits.check_measured_request(counts, u64::from(context), u64::from(output))
    }
}

/// Reads current authoritative availability/technical policy without inference,
/// tool connection, budget reservation or selection of another target.
#[async_trait]
pub trait ProviderDispatchGuard: std::fmt::Debug + Send + Sync {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError>;

    /// Effect-free compatibility of an already selected client. This supplies
    /// no counts and no permission to send. Actual dispatch must still call
    /// `check` and, where selected, the immutable-body final guard.
    async fn check_preparation(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        self.check(facts).await
    }
}

pub(crate) struct GuardedProvider {
    inner: Arc<dyn LlmProvider>,
    guard: Arc<dyn ProviderDispatchGuard>,
    cache: Option<Arc<crate::CachedProvider>>,
}

impl GuardedProvider {
    pub(crate) fn new(
        inner: Arc<dyn LlmProvider>,
        guard: Arc<dyn ProviderDispatchGuard>,
        cache: Option<Arc<crate::CachedProvider>>,
    ) -> Self {
        Self {
            inner,
            guard,
            cache,
        }
    }

    async fn check(
        &self,
        model: Option<&str>,
        max_output_tokens: Option<u32>,
    ) -> Result<(), LlmError> {
        let model = self.inner.effective_model_name(model);
        let state = self
            .guard
            .check(ProviderRequestFacts {
                model: &model,
                max_output_tokens,
            })
            .await?;
        if let Some(cache) = &self.cache {
            cache.apply_dispatch_state(state)?;
        }
        Ok(())
    }
}

#[async_trait]
impl LlmProvider for GuardedProvider {
    fn supports_final_request_guard(&self) -> bool {
        self.inner.supports_final_request_guard()
    }

    fn model_name(&self) -> std::borrow::Cow<'_, str> {
        self.inner.model_name()
    }
    fn active_model_name(&self) -> String {
        self.inner.active_model_name()
    }
    fn effective_model_name(&self, model: Option<&str>) -> String {
        self.inner.effective_model_name(model)
    }
    fn cost_per_token(&self) -> (Decimal, Decimal) {
        self.inner.cost_per_token()
    }
    fn cache_write_multiplier(&self) -> Decimal {
        self.inner.cache_write_multiplier()
    }
    fn cache_read_discount(&self) -> Decimal {
        self.inner.cache_read_discount()
    }
    fn calculate_cost(&self, input: u32, output: u32) -> Decimal {
        self.inner.calculate_cost(input, output)
    }
    fn set_model(&self, _model: &str) -> Result<(), LlmError> {
        // A database-selected client is immutable. Change the owning selection
        // and publish a successor rather than mutating another retained call.
        Err(LlmError::ConfigurationChanged {
            provider: "retained database provider".into(),
        })
    }

    async fn check_dispatch(
        &self,
        model: Option<&str>,
        max_output_tokens: Option<u32>,
    ) -> Result<(), LlmError> {
        self.check(model, max_output_tokens).await
    }
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.check(request.model.as_deref(), request.max_tokens)
            .await?;
        self.inner.complete(request).await
    }
    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        self.check(request.model.as_deref(), request.max_tokens)
            .await?;
        self.inner.complete_with_tools(request).await
    }
    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        self.check(None, None).await?;
        self.inner.list_models().await
    }
    async fn model_metadata(&self) -> Result<ModelMetadata, LlmError> {
        self.check(None, None).await?;
        self.inner.model_metadata().await
    }
}

#[cfg(test)]
mod token_policy_tests {
    use super::*;
    #[test]
    fn configured_comparison_binds_target_and_never_guesses_missing_limits_or_output() {
        let mut state = ProviderDispatchState {
            definition_revision: 1,
            response_cache: Default::default(),
            provider_id: "provider-a".into(),
            model: "shared-name".into(),
            model_token_limits: Some(ModelTokenLimits {
                max_prefix_tokens: 10,
                max_prompt_tokens: 20,
            }),
            configured_context_window_tokens: Some(35),
            requested_output_tokens: Some(5),
        };
        let counts = ModelRequestTokenCounts {
            prefix_tokens: 10,
            prompt_tokens: 20,
            input_tokens: 30,
        };
        assert_eq!(
            state.check_configured_token_counts("provider-a", "shared-name", counts),
            Ok(())
        );
        assert_eq!(
            state.check_configured_token_counts("provider-b", "shared-name", counts),
            Err(ModelTokenCheckError::TargetMismatch)
        );
        assert_eq!(
            state.check_configured_token_counts("provider-a", "other-model", counts),
            Err(ModelTokenCheckError::TargetMismatch)
        );
        state.requested_output_tokens = None;
        assert_eq!(
            state.check_configured_token_counts("provider-a", "shared-name", counts),
            Err(ModelTokenCheckError::UnknownOutputReservation)
        );
        state.requested_output_tokens = Some(0);
        assert_eq!(
            state.check_configured_token_counts("provider-a", "shared-name", counts),
            Err(ModelTokenCheckError::UnknownOutputReservation)
        );
        state.requested_output_tokens = Some(5);
        state.configured_context_window_tokens = None;
        assert_eq!(
            state.check_configured_token_counts("provider-a", "shared-name", counts),
            Err(ModelTokenCheckError::UnknownContext)
        );
        state.configured_context_window_tokens = Some(35);
        state.model_token_limits = None;
        assert_eq!(
            state.check_configured_token_counts("provider-a", "shared-name", counts),
            Err(ModelTokenCheckError::MissingInputLimits)
        );
        state.definition_revision = 0;
        assert_eq!(
            state.check_configured_token_counts("provider-a", "shared-name", counts),
            Err(ModelTokenCheckError::InvalidPolicy)
        );
    }
}
