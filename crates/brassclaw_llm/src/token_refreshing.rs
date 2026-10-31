//! Token-refreshing LlmProvider decorator for OpenAI Codex.
//!
//! Wraps an `OpenAiCodexProvider` and:
//! - Pre-emptively refreshes the OAuth access token before each call if near expiry
//! - Updates the inner provider's token after refresh (no client rebuild needed)
//! - Standalone clients may retry once on auth failure when request policy permits
//! - Host-managed clients propagate refresh failures before model submission and
//!   leave authentication repair to the host without replaying the model exchange
//! - Overrides `cost_per_token()` to return (0, 0) since billing is through subscription

use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use secrecy::ExposeSecret;

use crate::error::LlmError;
use crate::openai_codex_provider::OpenAiCodexProvider;
use crate::openai_codex_session::OpenAiCodexSessionManager;
use crate::provider::{
    CompletionRequest, CompletionResponse, LlmProvider, ModelMetadata, ToolCompletionRequest,
    ToolCompletionResponse,
};

/// Decorator that refreshes OAuth tokens before API calls and reports zero cost.
///
/// The inner `OpenAiCodexProvider` manages its own token state, so after a
/// refresh we just call `update_token()` -- no client rebuild is needed.
pub struct TokenRefreshingProvider {
    inner: Arc<OpenAiCodexProvider>,
    session: Arc<OpenAiCodexSessionManager>,
    host_managed_authentication: bool,
}

impl TokenRefreshingProvider {
    pub fn new(inner: Arc<OpenAiCodexProvider>, session: Arc<OpenAiCodexSessionManager>) -> Self {
        Self {
            inner,
            session,
            host_managed_authentication: false,
        }
    }

    /// Captured/server-owned clients never initiate a device login or replay
    /// a model exchange to repair authentication. The host owns that lifecycle.
    pub(crate) fn with_host_managed_authentication(mut self) -> Self {
        self.host_managed_authentication = true;
        self
    }

    /// Push a fresh token from the session manager into the inner provider.
    async fn update_inner_token(&self) -> Result<(), LlmError> {
        let token = self.session.get_access_token().await?;
        self.inner.update_token(token.expose_secret()).await?;
        tracing::debug!("Updated inner provider token after refresh");
        Ok(())
    }

    /// Refresh and synchronize the token before an API call.
    ///
    /// Host-managed clients propagate errors before model submission. Standalone
    /// clients preserve the legacy best-effort refresh and policy-controlled retry.
    async fn ensure_fresh_token(&self) -> Result<(), LlmError> {
        if self.host_managed_authentication {
            // Also synchronize a refresh completed by another caller before
            // its inner-token update. Errors stop before a model submission.
            return self.update_inner_token().await;
        }
        if self.session.needs_refresh().await {
            match self.session.refresh_tokens().await {
                Ok(()) => {
                    if let Err(e) = self.update_inner_token().await {
                        tracing::debug!(
                            "Pre-emptive token update failed: {e}, will retry on auth failure"
                        );
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "Pre-emptive token refresh failed: {e}, will retry on auth failure"
                    );
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl LlmProvider for TokenRefreshingProvider {
    fn supports_final_request_guard(&self) -> bool {
        self.inner.supports_final_request_guard()
    }

    fn model_name(&self) -> std::borrow::Cow<'_, str> {
        self.inner.model_name()
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        (Decimal::ZERO, Decimal::ZERO)
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.ensure_fresh_token().await?;

        match self.inner.complete(request.clone()).await {
            Err(LlmError::AuthFailed { .. } | LlmError::SessionExpired { .. })
                if !self.host_managed_authentication
                    && request.dispatch_policy.allows_resubmission() =>
            {
                tracing::debug!("Auth failure during complete(), refreshing and retrying once");
                self.session.handle_auth_failure().await?;
                self.update_inner_token().await?;
                self.inner.complete(request).await
            }
            other => other,
        }
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        self.ensure_fresh_token().await?;

        match self.inner.complete_with_tools(request.clone()).await {
            Err(LlmError::AuthFailed { .. } | LlmError::SessionExpired { .. })
                if !self.host_managed_authentication
                    && request.dispatch_policy.allows_resubmission() =>
            {
                tracing::debug!(
                    "Auth failure during complete_with_tools(), refreshing and retrying once"
                );
                self.session.handle_auth_failure().await?;
                self.update_inner_token().await?;
                self.inner.complete_with_tools(request).await
            }
            other => other,
        }
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        self.ensure_fresh_token().await?;
        self.inner.list_models().await
    }

    async fn model_metadata(&self) -> Result<ModelMetadata, LlmError> {
        self.ensure_fresh_token().await?;
        self.inner.model_metadata().await
    }

    fn active_model_name(&self) -> String {
        self.inner.model_name().to_string()
    }

    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        self.inner.effective_model_name(requested_model)
    }

    fn set_model(&self, model: &str) -> Result<(), LlmError> {
        self.inner.set_model(model)
    }

    fn calculate_cost(&self, _input_tokens: u32, _output_tokens: u32) -> Decimal {
        Decimal::ZERO
    }

    fn cache_write_multiplier(&self) -> Decimal {
        self.inner.cache_write_multiplier()
    }

    fn cache_read_discount(&self) -> Decimal {
        self.inner.cache_read_discount()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex_test_helpers::{make_test_jwt, test_codex_config};
    use crate::openai_codex_session::OpenAiCodexSessionManager;
    use tempfile::tempdir;

    fn make_provider_and_session() -> (TokenRefreshingProvider, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let config = test_codex_config(dir.path().join("session.json"));
        let jwt = make_test_jwt("acct_test");
        let inner = Arc::new(
            OpenAiCodexProvider::new(&config.model, &config.api_base_url, &jwt, 300)
                .expect("provider creation should succeed"),
        );
        let session = Arc::new(OpenAiCodexSessionManager::new(config).unwrap());
        (TokenRefreshingProvider::new(inner, session), dir)
    }

    #[test]
    fn test_model_name_delegates() {
        let (provider, _dir) = make_provider_and_session();
        assert_eq!(provider.model_name(), "gpt-5.3-codex");
    }

    #[test]
    fn test_cost_per_token_zero() {
        let (provider, _dir) = make_provider_and_session();
        let (input, output) = provider.cost_per_token();
        assert_eq!(input, Decimal::ZERO);
        assert_eq!(output, Decimal::ZERO);
    }

    #[test]
    fn test_calculate_cost_zero() {
        let (provider, _dir) = make_provider_and_session();
        assert_eq!(provider.calculate_cost(1000, 500), Decimal::ZERO);
    }

    #[test]
    fn test_active_model_name_delegates() {
        let (provider, _dir) = make_provider_and_session();
        assert_eq!(provider.active_model_name(), "gpt-5.3-codex");
    }
    #[tokio::test]
    async fn host_managed_refresh_failure_stops_all_model_entry_points() {
        use crate::ChatMessage;
        use crate::codex_test_helpers::codex_http_fixture;
        use crate::openai_codex_session::OpenAiCodexSession;
        let server = codex_http_fixture(|path, _| {
            assert_eq!(
                path, "/oauth/token",
                "refresh failure must precede model submission"
            );
            (401, r#"{"error":"private-refresh-error"}"#.into())
        })
        .await;
        let home = tempdir().unwrap();
        let mut config = test_codex_config(home.path().join("session.json"));
        config.auth_endpoint = server.endpoint.clone();
        config.api_base_url = server.endpoint.clone();
        let jwt = make_test_jwt("acct_original");
        let inner = Arc::new(
            OpenAiCodexProvider::new(&config.model, &config.api_base_url, &jwt, 5).unwrap(),
        );
        let session = Arc::new(OpenAiCodexSessionManager::new(config).unwrap());
        session
            .set_session(OpenAiCodexSession {
                access_token: jwt,
                refresh_token: "original-refresh".into(),
                expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
                created_at: chrono::Utc::now(),
            })
            .await;
        let provider =
            TokenRefreshingProvider::new(inner, session).with_host_managed_authentication();
        let request = || CompletionRequest::new(vec![ChatMessage::user("do not submit")]);
        for result in [
            provider.complete(request()).await.map(|_| ()),
            provider
                .complete_with_tools(ToolCompletionRequest::from_completion_request(
                    request(),
                    vec![],
                ))
                .await
                .map(|_| ()),
            provider.list_models().await.map(|_| ()),
            provider.model_metadata().await.map(|_| ()),
        ] {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("401"));
            assert!(!error.contains("private-refresh-error"));
        }
        assert_eq!(server.requests.lock().await.len(), 4);
        server.finish().await;
    }

    #[tokio::test]
    async fn host_managed_auth_rejection_never_replays_or_starts_login() {
        use crate::ChatMessage;
        use crate::codex_test_helpers::codex_http_fixture;
        use crate::openai_codex_session::OpenAiCodexSession;
        let server = codex_http_fixture(|path, _| {
            assert_eq!(
                path, "/responses",
                "model auth failure cannot start token/device exchange"
            );
            (
                401,
                r#"{"error":{"message":"expired","type":"authentication_error"}}"#.into(),
            )
        })
        .await;
        let home = tempdir().unwrap();
        let mut config = test_codex_config(home.path().join("session.json"));
        config.auth_endpoint = server.endpoint.clone();
        config.api_base_url = server.endpoint.clone();
        let jwt = make_test_jwt("acct_original");
        let inner = Arc::new(
            OpenAiCodexProvider::new(&config.model, &config.api_base_url, &jwt, 5).unwrap(),
        );
        let session = Arc::new(OpenAiCodexSessionManager::new(config).unwrap());
        session
            .set_session(OpenAiCodexSession {
                access_token: jwt,
                refresh_token: "original-refresh".into(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                created_at: chrono::Utc::now(),
            })
            .await;
        let provider =
            TokenRefreshingProvider::new(inner, session).with_host_managed_authentication();
        let request = || CompletionRequest::new(vec![ChatMessage::user("one exchange")]);
        assert!(provider.complete(request()).await.is_err());
        assert!(
            provider
                .complete_with_tools(ToolCompletionRequest::from_completion_request(
                    request(),
                    vec![]
                ))
                .await
                .is_err()
        );
        assert_eq!(
            server.requests.lock().await.len(),
            2,
            "each caller gets exactly its own response"
        );
        server.finish().await;
    }
}
