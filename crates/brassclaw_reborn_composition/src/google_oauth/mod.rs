use crate::oauth_provider_client::{ExchangeScopePolicy, HostOAuthProviderSpec};

pub(crate) fn google_provider_spec() -> HostOAuthProviderSpec {
    HostOAuthProviderSpec {
        provider_id: brassclaw_auth::GOOGLE_PROVIDER_ID,
        capability_id: "brassclaw_auth.google_oauth",
        token_endpoint: brassclaw_auth::GOOGLE_TOKEN_ENDPOINT,
        secret_handle_prefix: "google",
        resource: None,
        exchange_scope_policy: ExchangeScopePolicy::RequireProviderScope,
    }
}
