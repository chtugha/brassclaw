use async_trait::async_trait;

use brassclaw_auth::{
    AuthProductError, AuthProviderClient, OAuthProviderCallbackRequest, OAuthProviderExchange,
    OAuthProviderExchangeContext, OAuthProviderRefresh, OAuthProviderRefreshRequest,
    validate_provider_callback_request,
};

/// Explicit provider client used when durable product-auth storage is available
/// but no OAuth provider implementation has been composed for this process.
#[derive(Debug, Default)]
pub(crate) struct UnavailableAuthProviderClient;

#[async_trait]
impl AuthProviderClient for UnavailableAuthProviderClient {
    async fn prepare_callback(
        &self,
        _context: &brassclaw_auth::OAuthProviderExchangeContext,
        _request: &brassclaw_auth::OAuthProviderCallbackRequest,
    ) -> Result<brassclaw_auth::OAuthTokenDestinations, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    async fn prepare_refresh(
        &self,
        _request: &OAuthProviderRefreshRequest,
    ) -> Result<brassclaw_auth::OAuthProviderRefreshPreparation, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    async fn exchange_callback(
        &self,
        _context: OAuthProviderExchangeContext,
        request: OAuthProviderCallbackRequest,
    ) -> Result<OAuthProviderExchange, AuthProductError> {
        validate_provider_callback_request(&request)?;
        Err(AuthProductError::BackendUnavailable)
    }

    async fn refresh_token(
        &self,
        _request: OAuthProviderRefreshRequest,
        _preparation: brassclaw_auth::OAuthProviderRefreshPreparation,
    ) -> Result<OAuthProviderRefresh, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }
}
